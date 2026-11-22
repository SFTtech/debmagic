use super::dbus_policy::{Hit, is_excessively_broad, policy_hits};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct DbusPolicyExcessivelyBroad;

declare_rule! {
    /// A D-Bus policy allows sending to any destination, on any interface.
    ///
    /// Scans the policy files directly inside `etc/dbus-1/{session,system}.d`
    /// and `usr/share/dbus-1/{session,system}.d`. Reports each `<allow>` rule
    /// that is about sending (`send_*`) but names no `send_destination`, when
    /// it is not limited to an interface other than
    /// `org.freedesktop.DBus.Properties`. The root "agent" pattern
    /// (`user="root"` with an `<allow>`) is skipped. The extra is the
    /// normalized rule. Location is the policy file, with the 1-based rule
    /// number as its line.
    ///
    /// Reported as an error, after the unscoped send check of
    /// dbus-policy-without-send-destination; this is the security-relevant
    /// subset (cf. CVE-2014-8148).
    DbusPolicyExcessivelyBroad,
    code = "LN1221",
    tag = "dbus-policy-excessively-broad",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DbusPolicyExcessivelyBroad {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = ctx.data_files().map(inspect).unwrap_or_default();
        for hit in hits {
            ctx.diagnostic(Diagnostic::error(hit.rule).with_location(Location {
                path: hit.path,
                line: Some(hit.position as u32),
                column: None,
            }));
        }
    }
}

fn inspect(files: &FileTree) -> Vec<Hit> {
    policy_hits(files, is_excessively_broad)
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
            // Scoped to a destination.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow send_destination="com.example.Service"/></policy>"#,
                ),
            )]),
            // Denying is not excessive.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(r#"<policy context="default"><deny send_interface="x.y"/></policy>"#),
            )]),
            // Limited to another interface: the weaker dbus-policy-without-send-destination only.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow send_interface="com.example.Iface"/></policy>"#,
                ),
            )]),
            // The root agent pattern is skipped.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(r#"<policy user="root"><allow send_type="method_call"/></policy>"#),
            )]),
            // Only policy folders are checked.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new(r#"<policy context="default"><allow send_type="x"/></policy>"#),
            )]),
            // Not a send rule.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow own="com.example.Service"/></policy>"#,
                ),
            )]),
            // Symlinks are not files.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::symlink("/etc/dbus-1/foo.conf"),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow send_type="method_call"/></policy>"#,
                ),
            )]),
            // Properties interface does not limit the rule.
            tree(&[(
                "etc/dbus-1/session.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow send_interface="org.freedesktop.DBus.Properties"/></policy>"#,
                ),
            )]),
            // Non-root user with an allow.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(r#"<policy user="nobody"><allow send_type="signal"/></policy>"#),
            )]),
        ];
        BinaryPackageTester::new(DbusPolicyExcessivelyBroad, pass, fail).test_and_snapshot();
    }
}
