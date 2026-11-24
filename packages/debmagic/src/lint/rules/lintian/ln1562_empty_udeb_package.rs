use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::ln0007_empty_binary_package::{is_dummy, is_empty};

pub struct EmptyUdebPackage;

declare_rule! {
    /// The udeb ships nothing beyond its standard documentation.
    ///
    /// The udeb counterpart of `empty-binary-package` (LN0007): the same walk,
    /// the same standard files, the same transitional and meta package
    /// exemptions, and the same override and bug file exceptions. Only the
    /// Subject differs. Extra and Location are empty. Severity is info and the
    /// Tag is not selected by default, as in lintian.
    EmptyUdebPackage,
    code = "LN1562",
    tag = "empty-udeb-package",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for EmptyUdebPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let fields = ctx.installation_control().fields().clone();
        if is_dummy(&package, &fields) {
            return;
        }
        let architecture = fields.get("Architecture").unwrap_or_default().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if is_empty(files, &package, &architecture) {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn udeb(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
        .udeb()
    }

    #[test]
    fn binary() {
        let pass = vec![
            udeb(&["usr/lib/libfoo.so.1"]),
            udeb(&["usr/share/doc/example/copyright"]).field("Description", "dummy package"),
            udeb(&["usr/share/doc/example/copyright"]).field("Section", "debian-installer/tasks"),
            udeb(&["usr/share/doc/example/subdir/notes"]),
            // Bookkeeping counts only for its own architecture.
            udeb(&["usr/share/doc/example/changelog.Debian.amd64.gz"]).field("Architecture", "all"),
            udeb(&["usr/share/doc/example/buildinfo_amd64.gz"]).field("Architecture", "all"),
            // A binary package is not this Rule's Subject.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            udeb(&[]),
            udeb(&["usr/share/doc/example/copyright"]),
            udeb(&["usr/share/doc/example/README"]),
            // An override or bug file is the exception, so it does not count.
            udeb(&["usr/share/lintian/overrides/example"]),
            udeb(&["usr/share/doc/example/changelog.Debian.amd64.gz"])
                .field("Architecture", "amd64"),
            udeb(&["usr/share/doc/example/buildinfo_amd64.gz"]).field("Architecture", "amd64"),
        ];
        BinaryPackageTester::new(EmptyUdebPackage, pass, fail).test_and_snapshot();
    }
}
