use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::ln1367_ored_depends_on_obsolete_package::replacement;

/// Build relationship fields lintian's obsolete-package tags read.
const BUILD_FIELDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

pub struct OredBuildDependsOnObsoletePackage;

declare_rule! {
    /// An obsolete package is one alternative of an ORed build dependency
    /// whose first alternative is current, such as `foo | obsolete-pkg`.
    ///
    /// The Source paragraph's `Build-Depends`, `Build-Depends-Indep` and
    /// `Build-Depends-Arch`. This is the build-dependency form of
    /// `ored-depends-on-obsolete-package`: an obsolete alternative is reported
    /// unless it is the first alternative of its dependency or every
    /// alternative is obsolete (those are `build-depends-on-obsolete-package`).
    /// Extra is the field, the alternative as written, and ` => replacement`
    /// when the data file names one. Applies to the Source tree and the Source
    /// package.
    OredBuildDependsOnObsoletePackage,
    code = "LN1553",
    tag = "ored-build-depends-on-obsolete-package",
    default_selected = false,
    experimental = false,
}

impl SourceRule for OredBuildDependsOnObsoletePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let source = control.source();
            BUILD_FIELDS
                .iter()
                .filter_map(|field| Some((*field, source.relation(field)?)))
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
                                .filter(|alternative| !(all || alternative.package() == first))
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
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn build_depends(value: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\nBuild-Depends: {value}\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            build_depends("foo | bar"),
            build_depends("xbase-clients"),
            build_depends("xlibs-dev | foo"),
        ];
        let fail = vec![
            build_depends("foo | xbase-clients (>= 0.1)"),
            build_depends("foo | exim"),
            build_depends("foo | exim, bar"),
            build_depends("foo | xlibs-dev"),
        ];
        SourceTreeTester::new(OredBuildDependsOnObsoletePackage, pass, fail).test_and_snapshot();
    }
}
