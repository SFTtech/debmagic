use std::sync::OnceLock;

use regex::Regex;

use super::dbus_policy::{Hit, policy_hits};
use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct DbusPolicyAtConsole;

declare_rule! {
    /// A D-Bus policy file uses the deprecated `at_console` condition.
    ///
    /// Scans the policy files directly inside `etc/dbus-1/{session,system}.d`
    /// and `usr/share/dbus-1/{session,system}.d` and reports each `<policy>`
    /// rule (`<allow>` or `<deny>`) whose enclosing policy tests
    /// `at_console="true"`. The extra is the normalized rule, such as
    /// `<policy at_console="true"><allow send_destination="com.example.Foo"/>`.
    /// Location is the policy file, with the 1-based rule number as its line.
    /// Symlinks, empty files and non-UTF-8 files are skipped.
    DbusPolicyAtConsole,
    code = "LN1220",
    tag = "dbus-policy-at-console",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DbusPolicyAtConsole {
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
    static AT_CONSOLE: OnceLock<Regex> = OnceLock::new();
    let at_console = AT_CONSOLE
        .get_or_init(|| Regex::new(r#"at_console=['"]true"#).expect("valid at_console regex"));
    policy_hits(files, |rule| at_console.is_match(rule))
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
            // Without the deprecated condition.
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<busconfig><policy context="default"><allow send_destination="com.example.Service"/></policy></busconfig>"#,
                ),
            )]),
            // Only policy folders are checked.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new(r#"<policy at_console="true"><allow send_destination="x"/>"#),
            )]),
            // Empty file.
            tree(&[("etc/dbus-1/system.d/foo.conf", TestFile::new(""))]),
            // Non-UTF-8 file.
            tree(&[(
                "etc/dbus-1/system.d/foo.conf",
                TestFile::from_bytes(
                    b"\xff<policy at_console=\"true\"><allow send_destination=\"x\"/>".to_vec(),
                ),
            )]),
            // A symlink is not a file.
            tree(&[(
                "etc/dbus-1/system.d/foo.conf",
                TestFile::symlink("/usr/share/dbus-1/foo.conf"),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/dbus-1/system.d/foo.conf",
                TestFile::new(
                    r#"<policy at_console="true"><allow send_destination="com.example.Service"/></policy>"#,
                ),
            )]),
            // Single quotes, and a deny rule.
            tree(&[(
                "etc/dbus-1/session.d/foo.conf",
                TestFile::new(
                    "<policy at_console='true'><deny send_destination='com.example.Other'/></policy>",
                ),
            )]),
            // Two rules in one file, the second under a policy that is closed first.
            tree(&[(
                "usr/share/dbus-1/session.d/foo.conf",
                TestFile::new(
                    "<policy context=\"default\"><deny send_destination=\"a\"/></policy>\n<policy at_console=\"true\"><allow send_destination=\"b\"/></policy>",
                ),
            )]),
        ];
        BinaryPackageTester::new(DbusPolicyAtConsole, pass, fail).test_and_snapshot();
    }
}
