use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsIeeeData;

declare_rule! {
    /// The package ships an IEEE OUI or IAB database.
    ///
    /// A regular file whose path contains `/oui` or `/iab`, optionally prefixed
    /// by one character and a hyphen, with an optional `.txt`, `.idx`, or
    /// `.db` and an optional compressed extension from
    /// `files/compressed-file-extensions`. The source package `ieee-data` is
    /// exempt. Extra is empty. The path is the Location. Udebs are checked too.
    PackageInstallsIeeeData,
    code = "LN0416",
    tag = "package-installs-ieee-data",
    default_selected = true,
    experimental = false,
}

static IEEE: LazyLock<Regex> = LazyLock::new(|| {
    let extensions = crate::lint::data::lines(include_str!(
        "../../../data/files/compressed-file-extensions"
    ))
    .collect::<Vec<_>>()
    .join("|");
    Regex::new(&format!(
        r"/(?:[^/]-)?(?:oui|iab)(?:\.(?:txt|idx|db))?(?:\.(?:{extensions}))?\z"
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for PackageInstallsIeeeData {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "ieee-data" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            if entry
                .path()
                .to_str()
                .is_some_and(|path| IEEE.is_match(path))
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

    fn data(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("oui\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/doc/example/README"),
            data("usr/share/misc/oui.txt.gz").field("Source", "ieee-data"),
            BinaryPackageCase::from(vec![(
                "usr/share/misc/oui.txt".to_string(),
                TestFile::symlink("oui"),
            )]),
        ];
        let fail = vec![
            data("usr/share/files-ieee-data/oui.txt.gz"),
            data("usr/share/files-ieee-data/oui.idx"),
            data("usr/share/files-ieee-data/oui.db"),
            data("usr/share/files-ieee-data/iab.idx"),
            data("usr/share/misc/x-oui"),
        ];
        BinaryPackageTester::new(PackageInstallsIeeeData, pass, fail).test_and_snapshot();
    }
}
