use super::dbus_policy::{Hit, is_unscoped_send, policy_hits};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct DbusPolicyWithoutSendDestination;

declare_rule! {
    /// A D-Bus policy allows or denies sending without a `send_destination`.
    ///
    /// Scans the policy files directly inside `etc/dbus-1/{session,system}.d`
    /// and `usr/share/dbus-1/{session,system}.d`. Reports each `<allow>` or
    /// `<deny>` rule that is about sending (`send_*`) but names no
    /// `send_destination`. The root "agent" pattern (`user="root"` with an
    /// `<allow>`) is skipped. The extra is the normalized rule. Location is
    /// the policy file, with the 1-based rule number as its line.
    ///
    /// Rules that are also excessively broad (an `<allow>` not limited to an
    /// interface other than `org.freedesktop.DBus.Properties`) are reported
    /// here and again by dbus-policy-excessively-broad.
    DbusPolicyWithoutSendDestination,
    code = "LN1223",
    tag = "dbus-policy-without-send-destination",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DbusPolicyWithoutSendDestination {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = ctx.data_files().map(inspect).unwrap_or_default();
        for hit in hits {
            ctx.diagnostic(Diagnostic::warn(hit.rule).with_location(Location {
                path: hit.path,
                line: Some(hit.position as u32),
                column: None,
            }));
        }
    }
}

fn inspect(files: &FileTree) -> Vec<Hit> {
    policy_hits(files, is_unscoped_send)
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
            // Not about sending.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow own="com.example.Service"/></policy>"#,
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
            // Symlinks are not read.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::symlink("/etc/dbus-1/foo.conf"),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><deny send_type="method_call"/></policy>"#,
                ),
            )]),
            // Limited to another interface: reported here, not as excessively broad.
            tree(&[(
                "etc/dbus-1/session.d/foo.conf",
                TestFile::new(
                    r#"<policy context="default"><allow send_interface="com.example.Iface"/></policy>"#,
                ),
            )]),
            // Two rules, the second one unscoped, with the first one scoped.
            tree(&[(
                "usr/share/dbus-1/session.d/foo.conf",
                TestFile::new(
                    "<policy context=\"default\"><allow send_destination=\"a\"/></policy>\n<policy user=\"nobody\"><allow send_type=\"signal\"/></policy>",
                ),
            )]),
        ];
        BinaryPackageTester::new(DbusPolicyWithoutSendDestination, pass, fail).test_and_snapshot();
    }
}
