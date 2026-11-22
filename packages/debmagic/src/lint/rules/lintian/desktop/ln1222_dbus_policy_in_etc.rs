use super::dbus_policy::policy_children;
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct DbusPolicyInEtc;

declare_rule! {
    /// A D-Bus policy file is installed under `/etc`.
    ///
    /// Reports each entry directly inside `etc/dbus-1/{session,system}.d` as
    /// a policy that belongs in `usr/share/dbus-1/`, where the package
    /// ships its own policy and the administrator's `/etc` copy is not
    /// needed. Any kind of entry is reported, including symlinks and
    /// directories, as lintian's `children` are. The extra is empty and the
    /// Location is the path, with no line number.
    DbusPolicyInEtc,
    code = "LN1222",
    tag = "dbus-policy-in-etc",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DbusPolicyInEtc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = ctx.data_files().map(inspect).unwrap_or_default();
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn inspect(files: &FileTree) -> Vec<std::path::PathBuf> {
    policy_children(files)
        .into_iter()
        .filter(|entry| entry.path().starts_with("etc"))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(&[]),
            // Shipped under /usr/share.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new("<busconfig/>"),
            )]),
            // Under /etc, but not in a policy folder.
            tree(&[("etc/dbus-1/foo.conf", TestFile::new("<busconfig/>"))]),
            // Deeper than a direct child of the policy folder.
            tree(&[(
                "etc/dbus-1/system.d/sub/foo.conf",
                TestFile::new("<busconfig/>"),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "etc/dbus-1/system.d/foo.conf",
                TestFile::new("<busconfig/>"),
            )]),
            // Empty files are still reported.
            tree(&[("etc/dbus-1/session.d/foo.conf", TestFile::new(""))]),
            // Symlinks and directories too, as lintian's children are.
            tree(&[(
                "etc/dbus-1/system.d/foo.conf",
                TestFile::symlink("/usr/share/dbus-1/foo.conf"),
            )]),
            tree(&[("etc/dbus-1/system.d/sub", TestFile::directory())]),
        ];
        BinaryPackageTester::new(DbusPolicyInEtc, pass, fail).test_and_snapshot();
    }
}
