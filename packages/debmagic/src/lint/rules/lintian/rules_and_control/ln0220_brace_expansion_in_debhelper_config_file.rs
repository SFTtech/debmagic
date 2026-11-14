use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::SourceRule;

use super::rules_text::read_utf8;

pub struct BraceExpansionInDebhelperConfigFile;

declare_rule! {
    /// A debhelper filename config uses shell brace expansion.
    ///
    /// After `debian/rules` has shown a dh command, each direct child of
    /// `debian/` whose name (or the suffix after the last dot) is a filename
    /// config (`dirs`, `docs`, `examples`, `info`, `install`, `links`,
    /// `manpages`, `mime`, `sgmlcatalogs`, `sharedmimeinfo`, `wm`) is scanned
    /// as text. Blank lines are skipped. A line that starts with `#` is
    /// skipped when the declared compat level is 5 or newer; below that, the
    /// comment is text. The first `{a,b}` expansion on a line, not preceded by
    /// `\`, is the extra, and later lines of that file are not scanned.
    /// Location is the file and that physical line. Applies to the Source
    /// tree and Source package.
    BraceExpansionInDebhelperConfigFile,
    code = "LN0220",
    tag = "brace-expansion-in-debhelper-config-file",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$BRACE_EXPANSION`: comments are ignored from compat 5.
const BRACE_EXPANSION: u32 = 5;

const FILENAME_CONFIGS: &[&str] = &[
    "dirs",
    "docs",
    "examples",
    "info",
    "install",
    "links",
    "manpages",
    "mime",
    "sgmlcatalogs",
    "sharedmimeinfo",
    "wm",
];

static BRACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^\\])(\{(?:[^\s\\}]*?,)+[^\\}\s,]*,*\})").expect("valid regex")
});

impl SourceRule for BraceExpansionInDebhelperConfigFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let skip_comments = facts.level() >= BRACE_EXPANSION;
        let Some(files) = ctx.files() else {
            return;
        };
        for (path, text) in config_files(files) {
            if let Some((line, expansion)) = first_expansion(&text, skip_comments) {
                ctx.diagnostic(Diagnostic::warn(expansion).with_location(Location {
                    path,
                    line: Some(line),
                    column: None,
                }));
            }
        }
    }
}

fn config_files(files: &FileTree) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for entry in files.entries() {
        let path = entry.path();
        if path.parent() != Some(Path::new("debian")) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_filename_config(name) {
            continue;
        }
        let Some(text) = read_utf8(files, &path.to_string_lossy()) else {
            continue;
        };
        found.push((path.to_path_buf(), text));
    }
    found
}

fn is_filename_config(name: &str) -> bool {
    let category = match name.rsplit_once('.') {
        Some((prefix, suffix)) if !prefix.is_empty() && !suffix.is_empty() => suffix,
        Some((_, "")) => return false,
        _ => name,
    };
    FILENAME_CONFIGS.contains(&category)
}

fn first_expansion(text: &str, skip_comments: bool) -> Option<(u32, String)> {
    for (index, line) in text.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if line.trim().is_empty() {
            continue;
        }
        if skip_comments && line.starts_with('#') {
            continue;
        }
        if let Some(found) = BRACE.captures(line) {
            return Some((number, found[1].to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const CONTROL_13: &str = "\
Source: example\n\
Build-Depends: debhelper-compat (= 13)\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const RULES: &str = "#!/usr/bin/make -f\n\n%:\n\tdh $@\n";

    fn package(
        control: &str,
        rules: &str,
        files: &[(&'static str, &str)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut tree = vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ];
        for (path, text) in files {
            tree.push((path, TestFile::new(*text)));
        }
        tree
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(
                CONTROL,
                "#!/usr/bin/make -f\nbuild:\n\ttrue\n",
                &[("debian/example.install", "{foo,bar}\n")],
            ),
            package(
                CONTROL_13,
                RULES,
                &[
                    (
                        "debian/example.install",
                        "{foo}\n{good}.txt\n{}.txt\n{.txt\n",
                    ),
                    ("debian/escaped.install", "\\{foo,bar}\n"),
                    ("debian/spaced.install", "{foo, bar}\n"),
                    ("debian/note.txt", "{foo,bar}\n"),
                    ("debian/source/nested.install", "{foo,bar}\n"),
                    ("debian/comment.install", "# {foo,bar}\n"),
                ],
            ),
            package(CONTROL_13, RULES, &[("debian/foo.", "{foo,bar}\n")]),
        ];
        let fail = vec![
            package(
                CONTROL_13,
                RULES,
                &[("debian/example.install", "{foo,bar}\n")],
            ),
            package(CONTROL_13, RULES, &[("debian/a.install", "{,foo}\n")]),
            package(CONTROL_13, RULES, &[("debian/b.install", "{foo,}\n")]),
            package(CONTROL_13, RULES, &[("debian/c.install", "{,,foo}\n")]),
            package(CONTROL_13, RULES, &[("debian/d.install", "{,foo,}\n")]),
            package(CONTROL_13, RULES, &[("debian/docs", "{foo,bar,baz}\n")]),
            // Only the first expansion in the file. A blank line still counts.
            package(
                CONTROL_13,
                RULES,
                &[(
                    "debian/example.install",
                    "\n{foo,bar} {baz,qux}\n{nope,more}\n",
                )],
            ),
            // Compat 1 (nothing declared) and compat 4 still read `#` lines.
            package(
                CONTROL,
                RULES,
                &[("debian/example.install", "# {foo,bar}\n")],
            ),
            {
                let mut files = package(
                    CONTROL,
                    RULES,
                    &[("debian/example.install", "# {foo,bar}\n")],
                );
                files.push(("debian/compat", TestFile::new("4\n")));
                files
            },
            // A `#` after indentation is not a comment.
            package(
                CONTROL_13,
                RULES,
                &[("debian/example.install", " # {foo,bar}\n")],
            ),
            package(
                CONTROL_13,
                RULES,
                &[
                    ("debian/a.install", "{foo,bar}\n"),
                    ("debian/b.docs", "{baz,qux}\n"),
                ],
            ),
        ];
        SourceTreeTester::new(BraceExpansionInDebhelperConfigFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            CONTROL_13,
            RULES,
            &[("debian/example.install", "{foo}\n")],
        ))];
        let fail = vec![SourcePackageCase::from(package(
            CONTROL_13,
            RULES,
            &[("debian/example.install", "{foo,bar}\n")],
        ))];
        SourcePackageTester::new(BraceExpansionInDebhelperConfigFile, pass, fail)
            .test_and_snapshot();
    }
}
