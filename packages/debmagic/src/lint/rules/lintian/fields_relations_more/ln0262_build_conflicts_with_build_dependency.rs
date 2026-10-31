use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::SourceRule;

use super::overlap::{build_depends_all, comma_separated, dependency_implies, source_field_line};

pub struct BuildConflictsWithBuildDependency;

declare_rule! {
    /// A build-conflict is also a build-dependency.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` are
    /// taken together. Each item of `Build-Conflicts`, `Build-Conflicts-Indep`,
    /// and `Build-Conflicts-Arch` is reported when that union implies it: the
    /// same package name, and the dependency's architecture restriction and
    /// `:arch` qualifier are at least as general. An unversioned conflict is
    /// implied whatever version the dependency names. A versioned conflict is
    /// not reported. Extra is the conflict field and the item as written.
    /// Location is `debian/control` at that field. Applies to the Source tree
    /// and the Source package.
    BuildConflictsWithBuildDependency,
    code = "LN0262",
    tag = "build-conflicts-with-build-dependency",
    default_selected = true,
    experimental = false,
}

const CONFLICT_FIELDS: &[&str] = &[
    "Build-Conflicts",
    "Build-Conflicts-Indep",
    "Build-Conflicts-Arch",
];

struct Hit {
    field: &'static str,
    item: String,
}

impl SourceRule for BuildConflictsWithBuildDependency {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hits = ctx
            .debian_control()
            .map(|control| hits(control.source()))
            .unwrap_or_default();
        if hits.is_empty() {
            return;
        }
        let text = control_text(ctx);
        for hit in hits {
            let line = text
                .as_deref()
                .and_then(|text| source_field_line(text, hit.field));
            ctx.diagnostic(
                Diagnostic::error(format!("{} {}", hit.field, hit.item)).with_location(Location {
                    path: PathBuf::from("debian/control"),
                    line,
                    column: None,
                }),
            );
        }
    }
}

fn hits(fields: &ControlParagraph) -> Vec<Hit> {
    let depend = build_depends_all(fields);
    let mut hits = Vec::new();
    for field in CONFLICT_FIELDS {
        let Some(value) = fields.unfolded_value(field) else {
            continue;
        };
        for item in comma_separated(&value) {
            if dependency_implies(&depend, item) {
                hits.push(Hit {
                    field,
                    item: item.to_string(),
                });
            }
        }
    }
    hits
}

fn control_text(ctx: &mut SourceContext<'_>) -> Option<String> {
    let files = ctx.files()?;
    read_control(files)
}

fn read_control(files: &FileTree) -> Option<String> {
    let entry = files.get("debian/control")?;
    String::from_utf8(files.read(entry).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Conflicts", "foo")]),
            control(&[
                ("Build-Depends", "foo (= 3) [!amd64 !i386], bar"),
                ("Build-Conflicts", "foo [amd64 i386], xlibs-dev"),
            ]),
            control(&[("Build-Depends", "foo"), ("Build-Conflicts", "foo (>= 1)")]),
            control(&[("Build-Depends", "bar | foo"), ("Build-Conflicts", "foo")]),
            control(&[
                ("Build-Depends", "foo <nocheck>"),
                ("Build-Conflicts", "foo"),
            ]),
        ];
        let fail = vec![
            control(&[
                (
                    "Build-Depends",
                    "foo (= 3) [!amd64 !i386], bar, arch-test1 [i386], arch-test2 [!i386]",
                ),
                ("Build-Depends-Indep", "debmake"),
                (
                    "Build-Conflicts",
                    "foo [amd64 i386], bar [alpha test], xlibs-dev, arch-test1 [powerpc], arch-test2 [!sparc]",
                ),
                ("Build-Conflicts-Indep", "debmake [!powerpc]"),
            ]),
            control(&[
                ("Build-Depends", "bd-conflict"),
                ("Build-Conflicts", "bd-conflict"),
            ]),
            control(&[
                ("Build-Depends-Arch", "make"),
                ("Build-Conflicts-Arch", "make"),
            ]),
            control(&[("Build-Depends", "foo (>= 2)"), ("Build-Conflicts", "foo")]),
        ];
        SourceTreeTester::new(BuildConflictsWithBuildDependency, pass, fail).test_and_snapshot();
    }
}
