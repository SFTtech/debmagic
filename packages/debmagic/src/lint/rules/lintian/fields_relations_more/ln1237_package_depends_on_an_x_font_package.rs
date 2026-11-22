use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageDependsOnAnXFontPackage;

declare_rule! {
    /// `Pre-Depends` or `Depends` names an X Window System font package: a
    /// package whose name starts with `xfont`, other than `xfonts-utils` and
    /// `xfonts-encodings`. Policy 11.8.5 says such fonts should be
    /// `Recommends` or `Suggests`. Reads the Installation control. Each
    /// offending alternative is reported; Extra is `FIELD: alternative` as
    /// written in the field. Udebs are checked too.
    PackageDependsOnAnXFontPackage,
    code = "LN1237",
    tag = "package-depends-on-an-x-font-package",
    default_selected = true,
    experimental = false,
}

const EXEMPT: &[&str] = &["xfonts-utils", "xfonts-encodings"];

impl BinaryPackageRule for PackageDependsOnAnXFontPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut findings = Vec::new();
        let fields = ctx.installation_control().fields();
        for field in ["Pre-Depends", "Depends"] {
            let Some(relation) = fields.relation(field) else {
                continue;
            };
            for alternative in relation.alternatives() {
                let package = alternative.package();
                if package.starts_with("xfont") && !EXEMPT.contains(&package) {
                    findings.push(format!("{field}: {}", alternative.text()));
                }
            }
        }
        for finding in findings {
            ctx.diagnostic(Diagnostic::error(finding));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn depends(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Depends", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            depends("libc6, xfonts-utils, xfonts-encodings"),
            // Recommends and Suggests are the right place for a font.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Recommends", "xfont-a")
                .field("Suggests", "xfonts-base"),
            // Only a name that starts with `xfont` counts.
            depends("font-xfont, libxfont2"),
        ];
        let fail = vec![
            depends("xfont-a"),
            depends("libc6, xfonts-base (>= 1:1.0)"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Pre-Depends", "xfonts-75dpi | libc6"),
            depends("xfont-a").udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnAnXFontPackage, pass, fail).test_and_snapshot();
    }
}
