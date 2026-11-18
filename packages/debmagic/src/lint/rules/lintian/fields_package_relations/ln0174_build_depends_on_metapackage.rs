use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::BUILD_DEPENDS_FIELDS;
use super::metapackages::is_metapackage;

pub struct BuildDependsOnMetapackage;

declare_rule! {
    /// A build dependency names a metapackage.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` of
    /// `debian/control`. Every alternative whose package is in
    /// `fields/metapackages` counts. Extra is the field and the alternative
    /// as written.
    BuildDependsOnMetapackage,
    code = "LN0174",
    tag = "build-depends-on-metapackage",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnMetapackage {
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
                        .filter(|alternative| is_metapackage(alternative.package()))
                        .map(|alternative| format!("{field}: {}", alternative.text()))
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
            control("Build-Depends: libc6\n"),
            control("Build-Depends: xorg-server\n"),
            control("Build-Conflicts: xorg\n"),
            control("Build-Depends: exim4 | xorg-server\n"),
        ];
        let fail = vec![control(
            "Build-Depends: xorg-dev, ok | xorg, xorg-dev:any (>= 1)\n\
             Build-Depends-Indep: x-window-system\n\
             Build-Depends-Arch: mono-complete\n",
        )];
        SourceTreeTester::new(BuildDependsOnMetapackage, pass, fail).test_and_snapshot();
    }
}
