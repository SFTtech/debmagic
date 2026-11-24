use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::files;

pub struct MissingFileFromPotfilesIn;

declare_rule! {
    /// An entry in `POTFILES.in` names a file that is not a regular file.
    ///
    /// Only checked when the package uses po-debconf (see LN0908). Each line
    /// that is not a comment is read as lintian does: a `[type: ...]` prefix
    /// is dropped, and blank lines are skipped. The rest is resolved relative
    /// to `debian/`, following symbolic links inside the tree. Extra is the
    /// name as written; the Location is `POTFILES.in` at the line number.
    MissingFileFromPotfilesIn,
    code = "LN0904",
    tag = "missing-file-from-potfiles-in",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingFileFromPotfilesIn {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        if !files::has_translatable_template(tree) {
            return;
        }
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        let Some(potfiles) = files::open_file(tree, &Path::new(directory).join("POTFILES.in"))
        else {
            return;
        };
        let Ok(bytes) = tree.read(potfiles) else {
            return;
        };
        let path = files::path_buf(potfiles);

        let mut missing: Vec<(Option<u32>, String)> = Vec::new();
        for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
            if line.trim_ascii_start().starts_with(b"#") {
                continue;
            }
            let name = potfile_name(line);
            if name.is_empty() {
                continue;
            }
            let wanted = Path::new("debian").join(OsStr::from_bytes(name));
            if files::open_file(tree, &wanted).is_none() {
                missing.push((
                    u32::try_from(index + 1).ok(),
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
        }

        for (line, extra) in missing {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path: path.clone(),
                line,
                column: None,
            }));
        }
    }
}

/// The file name of a `POTFILES.in` line: everything after the last `]`
/// (lintian's `s/.*\]\s*//`), with leading whitespace removed.
fn potfile_name(line: &[u8]) -> &[u8] {
    match line.iter().rposition(|&byte| byte == b']') {
        Some(index) => line[index + 1..].trim_ascii_start(),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str =
        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";
    const ASKS: &str = "Template: example/foo\nType: string\nDescription: Ask\n Question.\n";

    fn tree(potfiles: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/templates".to_string(), TestFile::new(ASKS)),
            ("debian/po".to_string(), TestFile::directory()),
            ("debian/po/POTFILES.in".to_string(), TestFile::new(potfiles)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(""),
            tree("# comment\n\n[type: gettext/glade] templates\n"),
            tree("[type: gettext/glade]   templates\n"),
        ];
        let fail = vec![
            tree("# comment\nsome-other-file\n"),
            tree("templates\n[type: gettext/glade] missing.glade\n"),
            tree("debian-dir-missing\n"),
        ];
        SourceTreeTester::new(MissingFileFromPotfilesIn, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("templates\n"))];
        let fail = vec![SourcePackageCase::from(tree("some-other-file\n"))];
        SourcePackageTester::new(MissingFileFromPotfilesIn, pass, fail).test_and_snapshot();
    }
}
