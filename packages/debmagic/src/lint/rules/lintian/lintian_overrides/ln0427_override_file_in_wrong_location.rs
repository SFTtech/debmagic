use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct OverrideFileInWrongLocation;

declare_rule! {
    /// A Lintian override is installed where it will not be read.
    ///
    /// `usr/share/doc/<package>/override.lintian` (either case of `lintian`,
    /// optional `.gz`) and anything under
    /// `usr/share/lintian/overrides/<package>/`. Extra is empty. The path is
    /// the Location. Udebs are checked too.
    OverrideFileInWrongLocation,
    code = "LN0427",
    tag = "override-file-in-wrong-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OverrideFileInWrongLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = regex::escape(ctx.package_name());
        let pattern = Regex::new(&format!(
            r"^usr/share/doc/{package}/override\.[lL]intian(?:\.gz)?$|^usr/share/lintian/overrides/{package}/.+"
        ))
        .expect("valid regex");
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if entry
                .path()
                .to_str()
                .is_some_and(|path| pattern.is_match(path))
            {
                paths.push(entry.path().to_path_buf());
            }
        }
        paths.sort();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn placed(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("tag\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            placed("usr/share/lintian/overrides/example"),
            placed("usr/share/doc/other/override.lintian"),
            placed("usr/share/lintian/overrides/other/some-override"),
        ];
        let fail = vec![
            placed("usr/share/doc/example/override.lintian"),
            placed("usr/share/doc/example/override.Lintian.gz"),
            placed("usr/share/lintian/overrides/example/some-override"),
        ];
        BinaryPackageTester::new(OverrideFileInWrongLocation, pass, fail).test_and_snapshot();
    }
}
