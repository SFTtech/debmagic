use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightDoesNotReferToCommonLicenseFile;

static SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"common-licenses(/\S+)").expect("valid regex"));

fn common_license_without_path(contents: &str) -> Option<String> {
    if contents.contains("/usr/share/common-licenses/") {
        return None;
    }
    SUFFIX
        .captures(contents)
        .map(|captures| captures[1].to_string())
}

declare_rule! {
    /// The copyright file names a common license by a path that is not under
    /// `/usr/share/common-licenses/`.
    ///
    /// The first `common-licenses/<path>` in the file, unless the file also has
    /// `/usr/share/common-licenses/` somewhere. Extra is the path after
    /// `common-licenses`, starting with the slash. Udebs are not checked. Location is
    /// empty.
    CopyrightDoesNotReferToCommonLicenseFile,
    code = "LN0606",
    tag = "copyright-does-not-refer-to-common-license-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightDoesNotReferToCommonLicenseFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if let Some(extra) = common_license_without_path(&contents) {
            ctx.diagnostic(Diagnostic::warn(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed("See /usr/share/common-licenses/GPL-3\n")),
            BinaryPackageCase::from(installed("No license pointer\n")),
            BinaryPackageCase::from(installed("common-licenses\n")),
            BinaryPackageCase::from(installed(
                "See usr/share/common-licenses/GPL-3 and /usr/share/common-licenses/GPL-2\n",
            )),
            BinaryPackageCase::from(installed("See common-licenses/GPL-3\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("See usr/share/common-licenses/GPL-3\n")),
            BinaryPackageCase::from(installed("See common-licenses/Apache-2.0 or elsewhere\n")),
        ];
        BinaryPackageTester::new(CopyrightDoesNotReferToCommonLicenseFile, pass, fail)
            .test_and_snapshot();
    }
}
