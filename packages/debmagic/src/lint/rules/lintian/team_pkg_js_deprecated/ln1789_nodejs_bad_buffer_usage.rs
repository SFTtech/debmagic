use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

const JAVASCRIPT_TEAM: &str = "pkg-javascript-maintainers@lists.alioth.debian.org";

/// `new Buffer(` with ASCII word boundaries and whitespace, as Perl matches
/// the bytes of the file.
static NEW_BUFFER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bnew\s+Buffer\(").expect("valid regex"));

pub struct NodejsBadBufferUsage;

declare_rule! {
    /// A JavaScript file of a package maintained by the JavaScript team calls the
    /// deprecated `new Buffer(`.
    ///
    /// Binary packages whose Installation control `Maintainer` names
    /// `pkg-javascript-maintainers@lists.alioth.debian.org` only. Each installed
    /// file whose path ends in `.js` is read, following a symlink when it resolves
    /// to a regular file or hardlink inside the package (lintian's `is_open_ok`),
    /// and reported when its bytes match `new Buffer(` with `new` as a whole word
    /// and any ASCII whitespace between `new` and `Buffer`. The Location is the
    /// installed path, so a symlink is reported at its own path, and the extra is
    /// empty, as lintian's pointed hint has no extra. A directory, an empty file
    /// and a dangling symlink are not reported.
    NodejsBadBufferUsage,
    code = "LN1789",
    tag = "nodejs-bad-buffer-usage",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NodejsBadBufferUsage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let javascript_team = ctx
            .installation_control()
            .fields()
            .get("Maintainer")
            .is_some_and(|maintainer| maintainer.contains(JAVASCRIPT_TEAM));
        if !javascript_team {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found = Vec::<PathBuf>::new();
        for entry in files.entries() {
            if !entry.path().as_os_str().as_bytes().ends_with(b".js") {
                continue;
            }
            let Some(target) = follow_symlinks(files, entry) else {
                continue;
            };
            let Ok(bytes) = files.read(target) else {
                continue;
            };
            if !bytes.is_empty() && NEW_BUFFER.is_match(&bytes) {
                found.push(entry.path().to_path_buf());
            }
        }
        for path in found {
            ctx.diagnostic(Diagnostic::warn(String::new()).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const JAVASCRIPT: &str =
        "Debian JavaScript Maintainers <pkg-javascript-maintainers@lists.alioth.debian.org>";

    fn team(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files).field("Maintainer", JAVASCRIPT)
    }

    fn js(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("usr/share/nodejs/example/index.js", TestFile::new(content))]
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not the JavaScript team.
            BinaryPackageCase::from(js("new Buffer(10)"))
                .field("Maintainer", "Example <ex@example.com>"),
            BinaryPackageCase::from(js("new Buffer(10)")),
            team(js("const b = Buffer.from('x');")),
            team(js("const b = Buffer.alloc(10);")),
            // Not a whole word, or not with `(` right after `Buffer`.
            team(js("renew Buffer(10)")),
            team(js("new Buffer (10)")),
            team(js("new buffer(10)")),
            // Not a `.js` file.
            team(vec![(
                "usr/share/doc/example/readme.txt",
                TestFile::new("new Buffer(10)"),
            )]),
            // Empty file, and a directory named `.js`.
            team(vec![(
                "usr/share/nodejs/example/empty.js",
                TestFile::new(""),
            )]),
            team(vec![(
                "usr/share/nodejs/example/dir.js",
                TestFile::directory(),
            )]),
            // A dangling symlink and a symlink out of the package.
            team(vec![(
                "usr/share/nodejs/example/link.js",
                TestFile::symlink("missing.js"),
            )]),
            team(vec![(
                "usr/share/nodejs/example/link.js",
                TestFile::symlink("../../../../../etc/passwd"),
            )]),
            // Invalid UTF-8 without a match.
            team(vec![(
                "usr/share/nodejs/example/bytes.js",
                TestFile::from_bytes(b"\xff\xfe Buffer.from(x)".to_vec()),
            )]),
        ];
        let fail = vec![
            team(js("const b = new Buffer(10);")),
            // `\s` matches a line break between `new` and `Buffer`.
            team(js("const b = new\nBuffer(10);")),
            // A hardlink is a file.
            team(vec![(
                "usr/share/nodejs/example/hard.js",
                TestFile::new("new Buffer(1)").hardlink(),
            )]),
            // A symlink is reported at its own path, and so is its target.
            team(vec![
                (
                    "usr/share/nodejs/example/link.js",
                    TestFile::symlink("real.js"),
                ),
                (
                    "usr/share/nodejs/example/real.js",
                    TestFile::new("new Buffer(1)"),
                ),
            ]),
            // Invalid UTF-8 still matches on its bytes.
            team(vec![(
                "usr/share/nodejs/example/bytes.js",
                TestFile::from_bytes(b"\xffnew Buffer(1)".to_vec()),
            )]),
        ];
        BinaryPackageTester::new(NodejsBadBufferUsage, pass, fail).test_and_snapshot();
    }
}
