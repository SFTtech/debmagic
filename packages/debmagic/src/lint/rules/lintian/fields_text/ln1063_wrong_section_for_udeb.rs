use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct WrongSectionForUdeb;

declare_rule! {
    /// A udeb's `Section` is not `debian-installer`.
    ///
    /// Only udebs are checked (`.udeb` file name). Extra is the unfolded
    /// `Section` of the Installation control. An undeclared `Section` reads as
    /// empty, so it reports an empty extra, as lintian does. Deb packages are
    /// skipped, whatever their section.
    WrongSectionForUdeb,
    code = "LN1063",
    tag = "wrong-section-for-udeb",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for WrongSectionForUdeb {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let section = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
            .unwrap_or_default();
        if section != "debian-installer" {
            ctx.diagnostic(Diagnostic::warn(section));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn udeb(section: Option<&str>) -> BinaryPackageCase {
        let case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).udeb();
        match section {
            Some(section) => case.field("Section", section),
            None => case,
        }
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Section", "devel"),
            udeb(Some("debian-installer")),
        ];
        let fail = vec![
            udeb(Some("devel")),
            udeb(Some("debian-installer/net")),
            udeb(None),
        ];
        BinaryPackageTester::new(WrongSectionForUdeb, pass, fail).test_and_snapshot();
    }
}
