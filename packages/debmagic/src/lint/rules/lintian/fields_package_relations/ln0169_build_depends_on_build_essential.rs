use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::SOURCE_RELATION_FIELDS;

pub struct BuildDependsOnBuildEssential;

declare_rule! {
    /// A source relationship names `build-essential`.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, `Build-Depends-Arch`,
    /// `Build-Conflicts`, `Build-Conflicts-Indep`, and `Build-Conflicts-Arch`
    /// of `debian/control`. Every alternative whose package is
    /// `build-essential` counts, with or without a `:arch` qualifier or a
    /// version. Extra is the field name.
    BuildDependsOnBuildEssential,
    code = "LN0169",
    tag = "build-depends-on-build-essential",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnBuildEssential {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let fields = control.source();
            SOURCE_RELATION_FIELDS
                .iter()
                .filter_map(|field| Some((*field, fields.relation(field)?)))
                .flat_map(|(field, relation)| {
                    relation
                        .alternatives()
                        .filter(|alternative| alternative.package() == "build-essential")
                        .map(|_| field.to_string())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
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
            control("Build-Depends: debhelper\n"),
            control("Build-Depends: build-essential-utils\n"),
        ];
        let fail = vec![control(
            "Build-Depends: build-essential, build-essential (>= 12), other | build-essential:any\n\
             Build-Depends-Indep: build-essential\n\
             Build-Conflicts: build-essential <!nocheck>\n",
        )];
        SourceTreeTester::new(BuildDependsOnBuildEssential, pass, fail).test_and_snapshot();
    }
}
