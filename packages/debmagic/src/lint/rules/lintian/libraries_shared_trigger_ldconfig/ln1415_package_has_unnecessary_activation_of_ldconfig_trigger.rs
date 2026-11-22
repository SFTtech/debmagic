use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::libraries_shared_trigger_ldconfig::ln1414_lacks_ldconfig_trigger::{
    must_call_ldconfig, triggers_ldconfig,
};

pub struct PackageHasUnnecessaryActivationOfLdconfigTrigger;

declare_rule! {
    /// A Binary package activates libc's `ldconfig` trigger but installs no
    /// shared library into an `ldconfig` folder.
    ///
    /// The Check of `lacks-ldconfig-trigger`, the reverse: it reports when the
    /// `triggers` Control file has the line `activate-noawait ldconfig` and no
    /// Data file is one `ldconfig` must see (see that Rule for the criteria).
    /// Skipped for udebs. The hint has no extra and no Location.
    PackageHasUnnecessaryActivationOfLdconfigTrigger,
    code = "LN1415",
    tag = "package-has-unnecessary-activation-of-ldconfig-trigger",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageHasUnnecessaryActivationOfLdconfigTrigger {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || !triggers_ldconfig(ctx) {
            return;
        }
        if !must_call_ldconfig(ctx).is_empty() {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn library() -> TestFile {
        TestFile::from_bytes(TestElf::shared_object().soname("libfoo.so.1").build())
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Installs a library in an ldconfig folder, so the trigger is needed.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library())])
                .control_file("triggers", TestFile::new("activate-noawait ldconfig\n")),
            // No trigger.
            BinaryPackageCase::from(vec![("usr/share/doc/app/README", TestFile::new("x\n"))]),
            // A udeb is not checked.
            BinaryPackageCase::from(vec![("usr/share/doc/app/README", TestFile::new("x\n"))])
                .control_file("triggers", TestFile::new("activate-noawait ldconfig\n"))
                .udeb(),
            // A line that is not the whole directive does not count.
            BinaryPackageCase::from(vec![("usr/share/doc/app/README", TestFile::new("x\n"))])
                .control_file("triggers", TestFile::new("activate ldconfig\n")),
        ];
        let fail = vec![
            // Trigger activated, but the library is not in an ldconfig folder.
            BinaryPackageCase::from(vec![("usr/lib/foo/libfoo.so.1", library())])
                .control_file("triggers", TestFile::new("activate-noawait ldconfig\n")),
            // Trigger activated, no library at all.
            BinaryPackageCase::from(vec![("usr/share/doc/app/README", TestFile::new("x\n"))])
                .control_file(
                    "triggers",
                    TestFile::new("  activate-noawait   ldconfig  \n"),
                ),
        ];
        BinaryPackageTester::new(PackageHasUnnecessaryActivationOfLdconfigTrigger, pass, fail)
            .test_and_snapshot();
    }
}
