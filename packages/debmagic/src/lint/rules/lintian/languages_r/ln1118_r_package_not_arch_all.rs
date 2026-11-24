use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct RPackageNotArchAll;

declare_rule! {
    /// An R library package (`r-cran-*`, `r-bioc-*`, or `r-other-*`) that ships
    /// an R `DESCRIPTION` file under `usr/lib/R/` stating `NeedsCompilation: no`
    /// is not `Architecture: all`.
    ///
    /// Only installed files that are not directories count. The content must be
    /// valid UTF-8 with a line starting `NeedsCompilation: no` (case-sensitive,
    /// as lintian's `/^NeedsCompilation: no/m`). A missing `Architecture` field
    /// counts as not `all`. Extra is empty and there is no Location. Udebs are
    /// checked too.
    RPackageNotArchAll,
    code = "LN1118",
    tag = "r-package-not-arch-all",
    default_selected = true,
    experimental = false,
}

static NEEDS_COMPILATION_NO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^NeedsCompilation: no").expect("valid regex"));

const R_PACKAGE_PREFIXES: &[&str] = &["r-cran-", "r-bioc-", "r-other-"];

impl BinaryPackageRule for RPackageNotArchAll {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let has_r_files = ctx.data_files().is_some_and(|files| {
            files.entries().iter().any(|entry| {
                !entry.is_dir()
                    && is_r_description(entry.path().as_os_str().as_bytes())
                    && entry.is_file()
                    && files.read(entry).is_ok_and(|bytes| {
                        std::str::from_utf8(&bytes)
                            .is_ok_and(|text| NEEDS_COMPILATION_NO.is_match(text))
                    })
            })
        });
        if !has_r_files {
            return;
        }

        let name = ctx.package_name();
        let is_r_package = R_PACKAGE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix));
        if !is_r_package {
            return;
        }

        let all = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture")
            .as_deref()
            == Some("all");
        if !all {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// `^usr/lib/R/.*/DESCRIPTION$` on raw path bytes; the two anchors may not overlap.
fn is_r_description(path: &[u8]) -> bool {
    let prefix: &[u8] = b"usr/lib/R/";
    let suffix: &[u8] = b"/DESCRIPTION";
    path.len() >= prefix.len() + suffix.len() && path.starts_with(prefix) && path.ends_with(suffix)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn r_package(name: &str, architecture: Option<&str>, description: &[u8]) -> BinaryPackageCase {
        let path = format!("usr/lib/R/site-library/{name}/DESCRIPTION");
        let mut case =
            BinaryPackageCase::from(vec![(path, TestFile::from_bytes(description))]).package(name);
        if let Some(architecture) = architecture {
            case = case.field("Architecture", architecture);
        }
        case
    }

    #[test]
    fn binary() {
        let compiled = b"Package: foo\nNeedsCompilation: yes\n";
        let pure = b"Package: foo\nNeedsCompilation: no\n";
        let pass = vec![
            r_package("r-cran-foo", Some("all"), pure),
            r_package("r-cran-foo", Some("any"), compiled),
            r_package("foo", Some("any"), pure),
            r_package("r-cran-foo", Some("any"), &[0xff, 0xfe, b'\n']),
            BinaryPackageCase::from(vec![(
                "usr/lib/R/site-library/foo/DESCRIPTION",
                TestFile::symlink("/nonexistent"),
            )])
            .package("r-cran-foo")
            .field("Architecture", "any"),
            BinaryPackageCase::from(vec![(
                "usr/share/R/DESCRIPTION",
                TestFile::from_bytes(pure),
            )])
            .package("r-cran-foo")
            .field("Architecture", "any"),
        ];
        let fail = vec![
            r_package("r-cran-foo", Some("any"), pure),
            r_package("r-bioc-foo", Some("any"), pure),
            r_package("r-other-foo", Some("amd64"), pure),
            r_package("r-cran-foo", None, pure),
            r_package("r-cran-foo", Some("any"), pure).udeb(),
        ];
        BinaryPackageTester::new(RPackageNotArchAll, pass, fail).test_and_snapshot();
    }
}
