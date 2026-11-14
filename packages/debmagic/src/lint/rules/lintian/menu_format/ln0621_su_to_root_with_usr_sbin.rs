use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct SuToRootWithUsrSbin;

declare_rule! {
    /// A menu or desktop command starts with `/usr/sbin/su-to-root`.
    ///
    /// Only the exact first word `/usr/sbin/su-to-root` counts; the wrapper is
    /// installed in `/usr/bin`. No extra. Menu items use lintian's line;
    /// desktop entries (directly under `usr/share/applications`) do not.
    /// Udebs are checked too.
    SuToRootWithUsrSbin,
    code = "LN0621",
    tag = "su-to-root-with-usr-sbin",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SuToRootWithUsrSbin {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for finding in scan::take(ctx, |report| report.su_to_root_with_usr_sbin) {
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
            item("su-to-root -c hello"),
            item("/usr/bin/su-to-root -c hello"),
            item("hello /usr/sbin/su-to-root"),
            item("/usr/sbin/sux -c hello"),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/hello.desktop".to_string(),
                TestFile::new("[Desktop Entry]\nType=Application\nExec=su-to-root -X -c hello\n"),
            )]),
        ];
        let fail = vec![
            item("/usr/sbin/su-to-root -X -c hello"),
            item("/usr/sbin/su-to-root"),
            BinaryPackageCase::from(vec![(
                "usr/lib/menu/example".to_string(),
                TestFile::new(
                    "?package(example):needs=\"text\" section=\"Applications/System\" title=\"T\" command=\"/usr/sbin/su-to-root -c x\"\n",
                ),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/applications/hello.desktop".to_string(),
                TestFile::new(
                    "[Desktop Entry]\nType=Application\nExec=/usr/sbin/su-to-root -X -c hello\n",
                ),
            )]),
            item("/usr/sbin/su-to-root -c hello").udeb(),
        ];
        BinaryPackageTester::new(SuToRootWithUsrSbin, pass, fail).test_and_snapshot();
    }
}
