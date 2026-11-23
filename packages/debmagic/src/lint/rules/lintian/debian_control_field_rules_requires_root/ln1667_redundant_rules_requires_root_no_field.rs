use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct RedundantRulesRequiresRootNoField;

declare_rule! {
    /// The source paragraph of `debian/control` sets `Rules-Requires-Root: no`,
    /// which dpkg 1.22.13 and later already assume.
    ///
    /// Only the source paragraph is read; installable paragraphs are not. The
    /// Location is `debian/control` at the line of the field. The extra is
    /// empty. Severity is pedantic and the Tag is not selected by default, as
    /// in lintian. Applies to the Source tree and Source package.
    RedundantRulesRequiresRootNoField,
    code = "LN1667",
    tag = "redundant-rules-requires-root-no-field",
    default_selected = false,
    experimental = false,
}

impl SourceRule for RedundantRulesRequiresRootNoField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let line = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let source = control.source();
            if source.get("Rules-Requires-Root") != Some("no") {
                return;
            }
            source.position("Rules-Requires-Root")
        };
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Pedantic)
                .with_location(Location {
                    path: PathBuf::from("debian/control"),
                    line,
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(source_fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
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
        )]
    }

    #[test]
    fn test() {
        let pass = vec![control(""), control("Rules-Requires-Root: binary-targets")];
        let fail = vec![control("Rules-Requires-Root: no")];
        SourceTreeTester::new(RedundantRulesRequiresRootNoField, pass, fail).test_and_snapshot();
    }
}
