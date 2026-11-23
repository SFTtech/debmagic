use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

pub struct RulesSilentlyRequireRoot;

declare_rule! {
    /// An installable ships a file or directory owned by other than `root:root`,
    /// but the source does not declare `Rules-Requires-Root: binary-targets`.
    ///
    /// Reported when the source paragraph of `debian/control` has no
    /// `Rules-Requires-Root` field, or sets it to `no`. For each installable
    /// (udebs too), the first entry of its installed files in byte order of the
    /// path (a directory sorts with its trailing `/`) whose user or group is not
    /// `root` is reported: the extra is `<installable> (<owner>:<group>)
    /// <path>`, a directory path ends in `/`. One finding per installable. An
    /// owner with no name prints its number; uid or gid `0` is `root`. The
    /// Location is `debian/control` at the line of the field, or no line when
    /// the field is missing. Severity is info and the Tag is not selected by
    /// default, as in lintian. A Group Rule: it needs installables, so a bare
    /// Source tree never reports; the finding is on the source package.
    RulesSilentlyRequireRoot,
    code = "LN1775",
    tag = "rules-silently-require-root",
    default_selected = false,
    experimental = false,
}

const FIELD: &str = "Rules-Requires-Root";

impl GroupRule for RulesSilentlyRequireRoot {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        // lintian: `!declares || value eq 'no'`.
        if source.declares(FIELD) && source.get(FIELD) != Some("no") {
            return;
        }
        let line = source.position(FIELD);
        let mut reports = Vec::new();
        for installable in ctx.installables() {
            let Some(files) = ctx.data_files(&installable) else {
                continue;
            };
            let mut entries: Vec<&Entry> = files.entries().iter().collect();
            entries.sort_by_cached_key(|entry| lintian_name(entry).into_bytes());
            let found = entries.into_iter().find_map(|entry| {
                let owner = entry.owner()?;
                let (user, group) = (owner.user(), owner.group());
                (user != "root" || group != "root").then(|| {
                    format!(
                        "{} ({user}:{group}) {}",
                        installable.name(),
                        lintian_name(entry)
                    )
                })
            });
            reports.extend(found);
        }
        for extra in reports {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

/// An entry's name as lintian's `name` gives it: the path, with a trailing `/`
/// for a directory.
fn lintian_name(entry: &Entry) -> String {
    let path = String::from_utf8_lossy(entry.path().as_os_str().as_bytes()).into_owned();
    if entry.is_dir() {
        format!("{path}/")
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Owner;
    use crate::lint::tester::{GroupCase, GroupTester, TestCase, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    /// The source paragraph with `source_fields` (each line ending in `\n`).
    fn control(source_fields: &str) -> TestCase {
        TestCase::from(vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 {source_fields}\n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n"
            )),
        )])
    }

    fn user_file(path: &'static str) -> (&'static str, TestFile) {
        (path, TestFile::new("x").owned_by("user", "user"))
    }

    #[test]
    fn group() {
        let pass = vec![
            // Declared as binary-targets: not reported.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([user_file("usr/bin/example")])
                .source_files(control("Rules-Requires-Root: binary-targets\n")),
            // No field, but everything is root-owned.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([("usr/bin/example", TestFile::new("x"))])
                .source_files(control("")),
            // Uid and gid 0 without names are root.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([(
                    "usr/bin/example",
                    TestFile::new("x").owner(Owner::new(0, 0, None, None)),
                )])
                .source_files(control("")),
            // No installables: nothing to report.
            GroupCase::new(DSC).source_files(control("")),
            // An installable without files.
            GroupCase::new(DSC)
                .installable("example", "")
                .source_files(control("")),
        ];
        let fail = vec![
            // Field missing: no line.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([user_file("usr/bin/example")])
                .source_files(control("")),
            // `no` is reported at the field's line.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([user_file("usr/bin/example")])
                .source_files(control("Rules-Requires-Root: no\n")),
            // Only the group differs.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([(
                    "usr/bin/example",
                    TestFile::new("x").owned_by("root", "games"),
                )])
                .source_files(control("")),
            // A numeric owner prints its number.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([(
                    "usr/bin/example",
                    TestFile::new("x").owner(Owner::new(1000, 1000, None, None)),
                )])
                .source_files(control("")),
            // The first entry in byte order is the one reported; once per installable.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([
                    user_file("usr/share/doc/example/readme"),
                    user_file("usr/bin/example"),
                ])
                .source_files(control("")),
            // A directory is named with its trailing slash.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([(
                    "usr/share/example",
                    TestFile::directory().owned_by("user", "user"),
                )])
                .source_files(control("")),
            // Every installable is checked, and a udeb counts.
            GroupCase::new(DSC)
                .installable("example", "")
                .files([user_file("usr/bin/example")])
                .udeb("example-udeb", "")
                .files([user_file("usr/lib/example.so")])
                .source_files(control("")),
        ];
        GroupTester::new(RulesSilentlyRequireRoot, pass, fail).test_and_snapshot();
    }
}
