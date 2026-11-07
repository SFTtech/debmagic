use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct SuWrapperNotSuToRoot;

declare_rule! {
    /// A menu or desktop command uses a su wrapper other than `su-to-root`.
    ///
    /// `gksu`, `kdesu`, and `sux` are reported unless the wrapper name is the
    /// package name. An empty `gksu`/`kdesu` invocation clears the wrapper and
    /// is not this Tag. Extra is the wrapper. Menu items use lintian's line;
    /// desktop entries do not. Udebs are checked too.
    SuWrapperNotSuToRoot,
    code = "LN0429",
    tag = "su-wrapper-not-su-to-root",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SuWrapperNotSuToRoot {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.su_wrapper_not_su_to_root) {
            scan::warn(ctx, &finding);
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
            item("su-to-root cfdisk"),
            item("sux -p cfdisk").package("sux"),
            item("gksu"),
        ];
        let fail = vec![
            item("sux -p cfdisk"),
            item("kdesu hello"),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/hello.desktop".to_string(),
                TestFile::new("[Desktop Entry]\nType=Application\nExec=kdesu hello\n"),
            )]),
        ];
        BinaryPackageTester::new(SuWrapperNotSuToRoot, pass, fail).test_and_snapshot();
    }
}
