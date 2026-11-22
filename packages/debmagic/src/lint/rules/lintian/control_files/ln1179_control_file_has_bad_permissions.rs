use std::collections::HashMap;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ControlFileHasBadPermissions;

declare_rule! {
    /// A recognised control file does not have the permission bits its name calls for.
    ///
    /// Checks the regular files of the Control files of a Binary package
    /// (deb and udeb) that `data/control-files/deb-permissions` (or
    /// `udeb-permissions` for a udeb) lists, against the octal mode the table
    /// gives. The `control` file is skipped, as lintian skips it. Extra is
    /// the mode the file has and the mode expected, both as `%04o`, as
    /// `0755 != 0644`. The Location is the control file. Unknown names,
    /// hardlinks, directories and other non-regular entries are not judged
    /// here (see the sibling Rules).
    ControlFileHasBadPermissions,
    code = "LN1179",
    tag = "control-file-has-bad-permissions",
    default_selected = true,
    experimental = false,
}

static DEB_MODES: LazyLock<HashMap<&'static str, u32>> =
    LazyLock::new(|| modes(include_str!("../../../data/control-files/deb-permissions")));
static UDEB_MODES: LazyLock<HashMap<&'static str, u32>> =
    LazyLock::new(|| modes(include_str!("../../../data/control-files/udeb-permissions")));

/// Each line: the control file name, then its octal mode.
fn modes(text: &'static str) -> HashMap<&'static str, u32> {
    crate::lint::data::lines(text)
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?;
            let mode = u32::from_str_radix(words.next()?, 8).ok()?;
            Some((name, mode))
        })
        .collect()
}

impl BinaryPackageRule for ControlFileHasBadPermissions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let known = if ctx.is_udeb() {
            &*UDEB_MODES
        } else {
            &*DEB_MODES
        };
        let Some(files) = ctx.control_files() else {
            return;
        };
        let reports: Vec<Diagnostic> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_regular_file())
            .filter(|entry| entry.path().to_str() != Some("control"))
            .filter_map(|entry| {
                let expected = *known.get(entry.path().to_str()?)?;
                let actual = entry.mode();
                (actual != expected).then(|| {
                    Diagnostic::error(format!("{actual:04o} != {expected:04o}")).with_location(
                        Location {
                            path: entry.path().to_path_buf(),
                            line: None,
                            column: None,
                        },
                    )
                })
            })
            .collect();
        for diagnostic in reports {
            ctx.diagnostic(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(name, file)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case("config", TestFile::new("").mode(0o755)),
            case("postinst", TestFile::new("").mode(0o755)),
            case("md5sums", TestFile::new("").mode(0o644)),
            case("control", TestFile::new("Package: x\n").mode(0o755)),
            case("unknown", TestFile::new("").mode(0o755)),
            // A hardlink is not judged by this Rule.
            case("md5sums", TestFile::new("").mode(0o755).hardlink()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("isinstallable", TestFile::new("").mode(0o755)),
            // `udeb-permissions` does not list `md5sums`.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("md5sums", TestFile::new("").mode(0o755)),
        ];
        let fail = vec![
            case("config", TestFile::new("").mode(0o644)),
            case("md5sums", TestFile::new("").mode(0o755)),
            case("postinst", TestFile::new("").mode(0o644)),
            case("conffiles", TestFile::new("").mode(0o755)),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("config", TestFile::new("").mode(0o644)),
        ];
        BinaryPackageTester::new(ControlFileHasBadPermissions, pass, fail).test_and_snapshot();
    }
}
