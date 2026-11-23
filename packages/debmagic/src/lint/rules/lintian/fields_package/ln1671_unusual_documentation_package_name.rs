use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct UnusualDocumentationPackageName;

declare_rule! {
    /// The `Package` name of a Binary package ends in `-docs`, where the usual
    /// documentation suffix is `-doc`.
    ///
    /// Lintian's `fields/package` Check, on the Installation control's
    /// `Package`. Nothing is reported without a `Package` field. Extra is
    /// empty and the Location is empty. Severity is info and the Tag is not
    /// selected by default, as in lintian. Udebs are checked too.
    UnusualDocumentationPackageName,
    code = "LN1671",
    tag = "unusual-documentation-package-name",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for UnusualDocumentationPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        let Some(name) = fields.unfolded_value("Package") else {
            return;
        };
        if !name.ends_with("-docs") {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package(name)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            package("example-doc"),
            package("example-docs-tools"),
            package("docs-example"),
        ];
        let fail = vec![package("example-docs"), package("example-docs").udeb()];
        BinaryPackageTester::new(UnusualDocumentationPackageName, pass, fail).test_and_snapshot();
    }
}
