use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::is_non_free;
use crate::lint::rule::SourceRule;

use super::source_missing::{source_is_missing, visited_files};

pub struct SourceIsMissing;

declare_rule! {
    /// A file in the Source tree or Source package is built from a source that
    /// is not there: compiled Python, an ELF object, a Flash movie or project,
    /// a script that is minified or has a very long line, or a page with a very
    /// long line.
    ///
    /// lintian looks for the source next to the file (the name with its build
    /// suffix swapped for a source suffix, in the directory, its parent, `src/`
    /// and a few other places), in `.libs/`, `unpacked/`, `debian/` and in
    /// `debian/missing-sources/` (by path and by base name). Reported with an
    /// empty extra and the path as the Location: in lintian 2.144.0 the Check
    /// emits it only as a pointer.
    ///
    /// The file kinds are sniffed from the bytes, standing in for `file(1)`:
    /// the magic number of CPython code, `FWS`/`CWS`/`ZWS`/`FLV` for Flash, an
    /// OLE2 container named `*.fla` for a Flash project, and ELF (the File type,
    /// or else the ELF magic, as `file(1)` says `ELF` for corrupt headers). Like lintian's `find_source`, a hardlink always has a missing
    /// source, and so does every file of a non-free package (`Section` starting
    /// with `non-free/`, `non-free-firmware/`, `restricted/` or `multiverse/`):
    /// `find_source` returns nothing for both. Symlinks in the middle of a
    /// candidate path are not followed, and a file that is two kinds (an ELF
    /// object named `.min.js`) is reported once, where lintian reports it twice.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files). Quilt `.pc/` is filtered out and `debian/missing-sources/` is
    /// skipped.
    SourceIsMissing,
    code = "LN1390",
    tag = "source-is-missing",
    default_selected = false,
    experimental = true,
}

impl SourceRule for SourceIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let non_free = is_non_free(ctx);
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<_> = visited_files(files)
            .filter(|entry| source_is_missing(non_free, files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const NON_FREE_CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Section: non-free/misc\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const ELF: FileType = FileType::Elf {
        has_static_symbol_table: false,
        executable: true,
    };
    const PYTHON38: &[u8] = b"U\x0d\r\n\0\0\0\0rest";

    fn tree(extra: Vec<(&'static str, TestFile)>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![("debian/control", TestFile::from(CONTROL))];
        files.extend(extra);
        files
    }

    fn long_line() -> String {
        format!("var a = '{}';\n", "x".repeat(600))
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            // ELF with a source next to it, or one of the swapped suffixes.
            tree(vec![
                ("tool", TestFile::new("elf").file_type(ELF)),
                ("tool.c", TestFile::new("int main;")),
            ]),
            tree(vec![
                ("src/tool.o", TestFile::new("elf").file_type(ELF)),
                ("src/tool.S", TestFile::new("nop")),
            ]),
            tree(vec![
                ("lib/tool.o", TestFile::new("elf").file_type(ELF)),
                ("src/tool.c", TestFile::new("code")),
            ]),
            tree(vec![
                ("bin/tool", TestFile::new("elf").file_type(ELF)),
                ("src/tool.c", TestFile::new("code")),
            ]),
            tree(vec![
                ("bin/tool", TestFile::new("elf").file_type(ELF)),
                ("debian/missing-sources/tool.c", TestFile::new("code")),
            ]),
            // Python byte code with its source one directory up.
            tree(vec![
                (
                    "__pycache__/fake.cpython-38.pyc",
                    TestFile::from_bytes(PYTHON38.to_vec()),
                ),
                ("fake.py", TestFile::new("import fake")),
            ]),
            tree(vec![
                ("fake.pyc", TestFile::from_bytes(PYTHON38.to_vec())),
                (
                    "debian/missing-sources/fake.py",
                    TestFile::new("import fake"),
                ),
            ]),
            // Minified script with sources.
            tree(vec![
                ("usr/share/javascript/a/sourced.min.js", TestFile::new("a")),
                ("usr/share/javascript/a/sourced.js", TestFile::new("b")),
            ]),
            tree(vec![
                ("app.min.js", TestFile::new("a")),
                ("debian/missing-sources/app.min.js", TestFile::new("b")),
            ]),
            tree(vec![
                ("app.min.js", TestFile::new("a")),
                ("app-debug.js", TestFile::new("b")),
            ]),
            // Flash movie and project with an ActionScript source.
            tree(vec![
                ("movie.swf", TestFile::from_bytes(b"CWS\x09 x".to_vec())),
                ("movie.as", TestFile::new("code")),
            ]),
            tree(vec![
                (
                    "movie.fla",
                    TestFile::from_bytes(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".to_vec()),
                ),
                ("movie.as", TestFile::new("code")),
            ]),
            // A page and a script with no long line, text files, and a
            // composite document that is not a Flash project.
            tree(vec![
                ("index.html", TestFile::new("<html></html>")),
                ("a.js", TestFile::new("var a;")),
                (
                    "doc.xls",
                    TestFile::from_bytes(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".to_vec()),
                ),
            ]),
            // Sources of the skipped trees do not count, and the files in them are not checked.
            tree(vec![
                (".pc/tool", TestFile::new("elf").file_type(ELF)),
                (
                    "debian/missing-sources/tool",
                    TestFile::new("elf").file_type(ELF),
                ),
            ]),
            // The deployJava library with its `.txt`.
            tree(vec![
                (
                    "deployJava/deployJava.js",
                    TestFile::new("var deployJava = function() {};\n"),
                ),
                ("deployJava/deployJava.txt", TestFile::new("source")),
            ]),
            // A long line in a script with a `.debug.js` beside it.
            tree(vec![
                ("js/search.js", TestFile::new(long_line())),
                ("js/search.debug.js", TestFile::new("var a;")),
            ]),
            // A long line in a page with a fragment.
            tree(vec![
                ("index.html", TestFile::new(long_line())),
                ("index.html.fragment.js", TestFile::new("var a;")),
            ]),
            // A symlink to the source counts as the source.
            tree(vec![
                ("tool", TestFile::new("elf").file_type(ELF)),
                ("tool.c", TestFile::symlink("real.c")),
                ("real.c", TestFile::new("code")),
            ]),
        ];
        let fail = vec![
            tree(vec![("tool", TestFile::new("elf").file_type(ELF))]),
            // Corrupt section headers: no File type, but file(1) still says ELF.
            tree(vec![(
                "bin/tool",
                TestFile::from_bytes(b"\x7fELF\x02\x01\x01 corrupt headers".to_vec()),
            )]),
            tree(vec![
                ("bin/tool.o", TestFile::new("elf").file_type(ELF)),
                // A source for another file.
                ("bin/other.c", TestFile::new("code")),
            ]),
            // A file is its own "source" only for lintian when a suffix is
            // optional: the ELF pattern leaves `tool` as `tool`.
            tree(vec![
                ("tool", TestFile::new("elf").file_type(ELF)),
                ("tool.bak", TestFile::new("not a source")),
            ]),
            tree(vec![("fake2.pyc", TestFile::from_bytes(PYTHON38.to_vec()))]),
            tree(vec![
                (
                    "__pycache__/fake.cpython-38.pyc",
                    TestFile::from_bytes(PYTHON38.to_vec()),
                ),
                ("other/fake.py", TestFile::new("import fake")),
            ]),
            tree(vec![("fake.min.js", TestFile::new("a"))]),
            tree(vec![
                (
                    "usr/share/javascript/minwithoutsource/notsourced.min.js",
                    TestFile::new("a"),
                ),
                (
                    "usr/share/javascript/jswithoutminextwithoutsource/jsonnotsourced.js",
                    TestFile::new(long_line()),
                ),
            ]),
            // A minified script's source must not be itself.
            tree(vec![("a/test.min.js", TestFile::new("a"))]),
            tree(vec![(
                "fakeflash.swf",
                TestFile::from_bytes(b"FWS\x09 x".to_vec()),
            )]),
            tree(vec![(
                "fakefla.fla",
                TestFile::from_bytes(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".to_vec()),
            )]),
            tree(vec![(
                "movie.flv",
                TestFile::from_bytes(b"FLV\x01x".to_vec()),
            )]),
            tree(vec![(
                "deployJava/deployJava.js",
                TestFile::new("var deployJava = function() {};\n"),
            )]),
            tree(vec![("src/js/foo.html", TestFile::new(long_line()))]),
            tree(vec![(
                "oldfalsepositives/longlicensetext.js",
                TestFile::new(long_line()),
            )]),
            // A hardlink never has a source.
            tree(vec![
                ("tool", TestFile::new("elf").file_type(ELF).hardlink()),
                ("tool.c", TestFile::new("code")),
            ]),
            // Nor does a file of a non-free package.
            vec![
                ("debian/control", TestFile::from(NON_FREE_CONTROL)),
                ("tool", TestFile::new("elf").file_type(ELF)),
                ("tool.c", TestFile::new("code")),
            ],
        ];
        SourceTreeTester::new(SourceIsMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(vec![
            ("tool", TestFile::new("elf").file_type(ELF)),
            ("tool.c", TestFile::new("code")),
        ]))];
        let fail = vec![SourcePackageCase::from(tree(vec![(
            "tool",
            TestFile::new("elf").file_type(ELF),
        )]))];
        SourcePackageTester::new(SourceIsMissing, pass, fail).test_and_snapshot();
    }
}
