use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::BUILD_DEPENDS_FIELDS;
use super::obsolete_packages::replacement;

pub struct BuildDependsOnObsoletePackage;

declare_rule! {
    /// A build-dependency alternative names a package lintian lists as obsolete.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` of
    /// `debian/control`. The alternative is this Tag when it is the first of
    /// its dependency or every alternative is obsolete. Extra is the field,
    /// the alternative as written, and ` => replacement` when the data file
    /// names one.
    BuildDependsOnObsoletePackage,
    code = "LN0173",
    tag = "build-depends-on-obsolete-package",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnObsoletePackage {
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
                        .dependencies()
                        .iter()
                        .flat_map(|dependency| {
                            let obsolete: Vec<_> = dependency
                                .iter()
                                .filter(|alternative| replacement(alternative.package()).is_some())
                                .collect();
                            let first = dependency
                                .first()
                                .map(|alternative| alternative.package())
                                .unwrap_or("");
                            let all = !dependency.is_empty() && obsolete.len() == dependency.len();
                            obsolete
                                .into_iter()
                                .filter(|alternative| all || alternative.package() == first)
                                .map(|alternative| {
                                    let suffix = replacement(alternative.package())
                                        .filter(|value| !value.is_empty())
                                        .map(|value| format!(" => {value}"))
                                        .unwrap_or_default();
                                    format!("{field}: {}{suffix}", alternative.text())
                                })
                                .collect::<Vec<_>>()
                        })
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
            control("Build-Depends: libc6\n"),
            control("Build-Depends: current | gaim\n"),
            control("Build-Depends: libc6 | gaim\n"),
            control("Build-Conflicts: gaim\n"),
        ];
        let fail = vec![control(
            "Build-Depends: x-dev (>= 1.0), gaim | gs, gaim | libc6, \
             hardening-wrapper (>= 2.2), openjdk-6-jdk, pkg-config\n\
             Build-Depends-Indep: gs\n\
             Build-Depends-Arch: ruby1.8\n",
        )];
        SourceTreeTester::new(BuildDependsOnObsoletePackage, pass, fail).test_and_snapshot();
    }
}
