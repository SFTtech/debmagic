use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct SuWrapperWithoutC;

declare_rule! {
    /// A menu or desktop command uses `su-to-root`, `sux`, `gksu`, or `kdesu`
    /// without a command.
    ///
    /// `gksu` and `kdesu` take a trailing argument as the command. An empty
    /// `gksu`/`kdesu` invocation is not this Tag. Extra is the wrapper name.
    /// Menu items use lintian's line; desktop entries do not. Udebs are
    /// checked too.
    SuWrapperWithoutC,
    code = "LN0417",
    tag = "su-wrapper-without--c",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SuWrapperWithoutC {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.su_wrapper_without_c) {
            scan::error(ctx, &finding);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn menu(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/menu/example", TestFile::new(body))])
    }

    fn item(command: &str) -> BinaryPackageCase {
        menu(&format!(
            "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"{command}\"\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            item("/usr/bin/hello"),
            item("su-to-root -c hello"),
            item("kdesu hello"),
            item("gksu"),
            menu("?package(example):needs=\"text\" title=\"T\" command=\"sux cfdisk\"\n"),
        ];
        let fail = vec![
            item("/usr/bin/su-to-root cfdisk"),
            item("sux -p cfdisk"),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/hello.desktop".to_string(),
                TestFile::new("[Desktop Entry]\nType=Application\nExec=sux tool\n"),
            )]),
        ];
        BinaryPackageTester::new(SuWrapperWithoutC, pass, fail).test_and_snapshot();
    }
}
