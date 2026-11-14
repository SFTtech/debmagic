use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::ln0394_file_in_usr_lib_sgml::{at, matching};

pub struct PackageInstallsIntoEtcGconfSchemas;

declare_rule! {
    /// A file is installed under `etc/gconf/schemas/`.
    ///
    /// Any Data files entry whose path matches `etc/gconf/schemas/` plus one
    /// non-whitespace character. The directory itself is not flagged. Extra is
    /// empty. The Location is the path. Udebs are checked.
    PackageInstallsIntoEtcGconfSchemas,
    code = "LN0400",
    tag = "package-installs-into-etc-gconf-schemas",
    default_selected = true,
    experimental = false,
}

static SCHEMAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/gconf/schemas/\S").expect("regex"));

impl BinaryPackageRule for PackageInstallsIntoEtcGconfSchemas {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in matching(files, &SCHEMAS) {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(path)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("schema"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file("etc/gconf/schemas"),
            file("usr/share/gconf/schemas/test.schema"),
            file("etc/gconf/schema/test.schema"),
        ];
        let fail = vec![
            file("etc/gconf/schemas/test.schema"),
            file("etc/gconf/schemas/bar"),
            file("etc/gconf/schemas/nested/test.schema").udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsIntoEtcGconfSchemas, pass, fail)
            .test_and_snapshot();
    }
}
