use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::repeated_library_versions;

pub struct PackageDependsOnMultipleLibpngVersions;

declare_rule! {
    /// One relationship field depends on more than one libpng SONAME.
    ///
    /// The first alternative of each dependency in `Depends`, `Pre-Depends`,
    /// `Recommends`, and `Suggests`, counted separately per field. Later
    /// alternatives and an `:arch` qualifier are ignored. Extra is those
    /// package names. Applies to debs and udebs.
    PackageDependsOnMultipleLibpngVersions,
    code = "LN0263",
    tag = "package-depends-on-multiple-libpng-versions",
    default_selected = true,
    experimental = false,
}

const LIBPNG: &[&str] = &["libpng12-0", "libpng2", "libpng3"];

impl BinaryPackageRule for PackageDependsOnMultipleLibpngVersions {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = repeated_library_versions(ctx.installation_control().fields(), LIBPNG);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
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
            fields(&[("Depends", "libpng2")]),
            fields(&[("Depends", "libpng16-16, libpng16-16")]),
            fields(&[("Depends", "libc6 | libpng2, libpng3")]),
            fields(&[("Depends", "libpng2 | libpng3")]),
            fields(&[("Conflicts", "libpng2, libpng3")]),
            fields(&[("Depends", "libpng2"), ("Pre-Depends", "libpng3")]),
        ];
        let fail = vec![
            fields(&[("Depends", "libpng12-0, libpng2, libpng3")]),
            fields(&[("Depends", "libpng2:any, libpng3")]),
            fields(&[
                ("Depends", "libpng2, libpng3"),
                ("Suggests", "libpng12-0, libpng3"),
            ]),
            fields(&[("Depends", "libpng2, libpng3")]).udeb(),
        ];
        BinaryPackageTester::new(PackageDependsOnMultipleLibpngVersions, pass, fail)
            .test_and_snapshot();
    }
}
