use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, visit};

pub struct PackageContainsLindaOverride;

declare_rule! {
    /// A path under `usr/share/linda/overrides/`.
    ///
    /// Something must follow the slash. The overrides directory itself is
    /// not flagged. Any entry kind; directories keep a trailing slash.
    /// Udebs are checked too. Extra is empty.
    PackageContainsLindaOverride,
    code = "LN0215",
    tag = "package-contains-linda-override",
    default_selected = true,
    experimental = false,
}

static LINDA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/linda/overrides/\S+").expect("valid regex"));

impl BinaryPackageRule for PackageContainsLindaOverride {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(_, name)| LINDA.is_match(&name).then_some(name))
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            file("usr/share/linda/overrides", TestFile::directory()),
            file("usr/share/linda/override/pkg", TestFile::new("x")),
            file("usr/share/linda/overridesomething", TestFile::new("x")),
        ];
        let fail = vec![
            file("usr/share/linda/overrides/filenames", TestFile::new("x")),
            file("usr/share/linda/overrides/pkg", TestFile::directory()),
            file("usr/share/linda/overrides/pkg/extra", TestFile::new("x")),
            file(
                "usr/share/linda/overrides/link",
                TestFile::symlink("elsewhere"),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsLindaOverride, pass, fail).test_and_snapshot();
    }
}
