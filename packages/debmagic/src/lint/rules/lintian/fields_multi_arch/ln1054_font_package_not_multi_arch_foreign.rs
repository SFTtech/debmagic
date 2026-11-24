use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct FontPackageNotMultiArchForeign;

declare_rule! {
    /// A font package does not declare `Multi-Arch: foreign` or `allowed`.
    ///
    /// Applies to a binary package whose name starts with `fonts-` or
    /// `xfonts-`. A missing `Multi-Arch` counts as `no`. Extra is empty.
    /// Udebs are not skipped, as in lintian.
    FontPackageNotMultiArchForeign,
    code = "LN1054",
    tag = "font-package-not-multi-arch-foreign",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FontPackageNotMultiArchForeign {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let name = ctx.package_name();
        if !(name.starts_with("fonts-") || name.starts_with("xfonts-")) {
            return;
        }
        let fields = ctx.installation_control().fields();
        let multi = fields
            .unfolded_value("Multi-Arch")
            .unwrap_or_else(|| "no".to_string());
        if matches!(multi.as_str(), "foreign" | "allowed") {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn font(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new()).package(name)
    }

    #[test]
    fn binary() {
        let pass = vec![
            font("libfoo"),
            font("libfoo").field("Multi-Arch", "no"),
            font("fonts-foo").field("Multi-Arch", "foreign"),
            font("fonts-foo").field("Multi-Arch", "allowed"),
            font("xfonts-foo").field("Multi-Arch", "foreign"),
            font("fonts-foo").field("Multi-Arch", "foreign").udeb(),
        ];
        let fail = vec![
            font("fonts-foo"),
            font("fonts-foo").field("Multi-Arch", "no"),
            font("xfonts-foo").field("Multi-Arch", "same"),
            font("fonts-foo").udeb(),
        ];
        BinaryPackageTester::new(FontPackageNotMultiArchForeign, pass, fail).test_and_snapshot();
    }
}
