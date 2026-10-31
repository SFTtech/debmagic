use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct DependsExclusivelyOnMakedev;

declare_rule! {
    /// `Depends` is one alternative, and that package is `makedev`.
    ///
    /// Installation control. A version constraint stays on the same package.
    /// Extra is `Depends`. Applies to debs and udebs.
    DependsExclusivelyOnMakedev,
    code = "LN0171",
    tag = "depends-exclusively-on-makedev",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DependsExclusivelyOnMakedev {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            fields
                .relation("Depends")
                .map(|relation| {
                    relation
                        .dependencies()
                        .iter()
                        .filter(|dependency| {
                            dependency.len() == 1
                                && dependency
                                    .first()
                                    .is_some_and(|alternative| alternative.package() == "makedev")
                        })
                        .map(|_| "Depends".to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "libc6")]),
            fields(&[("Depends", "makedev | udev")]),
            fields(&[("Depends", "udev | makedev")]),
            fields(&[("Depends", "makedev (>= 1) | udev")]),
            fields(&[("Pre-Depends", "makedev")]),
            fields(&[("Recommends", "makedev")]),
            fields(&[("Depends", "makedev-extra")]),
            fields(&[("Depends", "makedev:any | udev")]),
        ];
        let fail = vec![
            fields(&[("Depends", "makedev, makedev (>= 1)")]),
            fields(&[("Depends", "other, makedev")]),
            fields(&[("Depends", "makedev:any")]),
            fields(&[("Depends", "makedev")]).udeb(),
        ];
        BinaryPackageTester::new(DependsExclusivelyOnMakedev, pass, fail).test_and_snapshot();
    }
}
