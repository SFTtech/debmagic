use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::BUILD_DEPENDS_FIELDS;

pub struct BuildDependsOn1Revision;

declare_rule! {
    /// A build dependency constrains a version with `>=` and a `-1` revision.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` of
    /// `debian/control`. The operator text is `>=` and the version text ends
    /// with `-1` (`1.2-1`, `2:3.0-1`). Extra is the field and the alternative
    /// as written.
    BuildDependsOn1Revision,
    code = "LN0170",
    tag = "build-depends-on-1-revision",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOn1Revision {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let fields = control.source();
            BUILD_DEPENDS_FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .alternatives()
                        .filter(|alternative| {
                            alternative.version().is_some_and(|version| {
                                version.operator == ">=" && version.version.ends_with("-1")
                            })
                        })
                        .map(|alternative| format!("{field}: {}", alternative.text()))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![],
            control(""),
            control("Build-Depends: libfoo (>= 1.2)\n"),
            control("Build-Depends: libfoo (>> 1.2-1)\n"),
            control("Build-Depends: libfoo (= 1.0-1)\n"),
            control("Build-Depends: libfoo (>= 1.0-10)\n"),
            control("Build-Depends: libfoo (>= 1.0-21)\n"),
            control("Build-Depends: libfoo (>= 1.2-1~)\n"),
            control("Build-Depends: libfoo (>= 1.0-1.1)\n"),
            control("Build-Depends: libfoo (>= 1.0-1+b1)\n"),
            control("Build-Conflicts: libfoo (>= 1.0-1)\n"),
        ];
        let fail = vec![control(
            "Build-Depends: revision-1 (>= 1.0-1), ok | edge (>= 3-1), pkg (>= 2:1.0-1) [amd64]\n\
             Build-Depends-Indep: libfoo (>= 1.2-1)\n\
             Build-Depends-Arch: other (>= 9-1)\n",
        )];
        SourceTreeTester::new(BuildDependsOn1Revision, pass, fail).test_and_snapshot();
    }
}
