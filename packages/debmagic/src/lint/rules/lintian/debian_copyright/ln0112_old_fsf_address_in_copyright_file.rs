use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::installed_copyright;

pub struct OldFsfAddressInCopyrightFile;

declare_rule! {
    /// The copyright file still has an old FSF postal address.
    ///
    /// Matches `Free Software Foundation` through `02139`, or the zip codes
    /// `02111-1307` and `02110-1301`, across newlines. Extra and Location are
    /// empty. Udebs are not checked.
    OldFsfAddressInCopyrightFile,
    code = "LN0112",
    tag = "old-fsf-address-in-copyright-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OldFsfAddressInCopyrightFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let found = installed_copyright::inspect(files, &package);
        if found.old_fsf {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(entries: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("Cambridge, MA 02139 without the foundation"),
            ),
        ])];
        let fail = vec![case(vec![
            ("usr/share/doc/example", TestFile::directory()),
            (
                "usr/share/doc/example/copyright",
                TestFile::new("Free Software Foundation, 51 Franklin St, Boston, MA 02110-1301"),
            ),
        ])];
        BinaryPackageTester::new(OldFsfAddressInCopyrightFile, pass, fail).test_and_snapshot();
    }
}
