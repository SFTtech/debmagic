use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct PackageContainsEmptyDirectory;

declare_rule! {
    /// A directory in the Binary package holds nothing: no entry has it as
    /// its parent.
    ///
    /// Skipped for the `base-files` package, for anything under `var/` or
    /// `etc/`, for the Perl directories `usr/lib/perl5/`,
    /// `usr/lib/<dir>/perl5/` and `usr/share/perl5/` that ExtUtils::MakeMaker
    /// leaves behind. Extra is empty; the directory is the Location. Severity
    /// is info and the Tag is not selected by default, as in lintian.
    PackageContainsEmptyDirectory,
    code = "LN1383",
    tag = "package-contains-empty-directory",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsEmptyDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "base-files" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let parents: HashSet<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| entry.path().parent().map(Path::to_path_buf))
            .collect();
        let hits: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::Directory])
            .into_iter()
            .filter(|entry| !parents.contains(entry.path()) && !is_skipped(entry.path()))
            .map(|entry| PathBuf::from(format!("{}/", entry.path().display())))
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_skipped(path: &Path) -> bool {
    let Some(name) = path.to_str().map(|name| name.trim_end_matches('/')) else {
        return false;
    };
    if name.starts_with("var/") || name.starts_with("etc/") {
        return true;
    }
    if name == "usr/share/perl5" {
        return true;
    }
    // usr/lib/perl5 or usr/lib/<dir>/perl5
    match name.strip_prefix("usr/lib/") {
        Some(rest) => {
            rest == "perl5"
                || rest
                    .split_once('/')
                    .is_some_and(|(dir, tail)| tail == "perl5" && !dir.is_empty())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn dir() -> TestFile {
        TestFile::directory()
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not empty.
            BinaryPackageCase::from(vec![
                ("usr/share/foo/", dir()),
                ("usr/share/foo/file", TestFile::new("x")),
            ]),
            // Skipped directories.
            BinaryPackageCase::from(vec![("var/lib/foo/", dir())]),
            BinaryPackageCase::from(vec![("etc/foo/", dir())]),
            BinaryPackageCase::from(vec![("usr/lib/perl5/", dir())]),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/perl5/", dir())]),
            BinaryPackageCase::from(vec![("usr/share/perl5/", dir())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/", dir())]),
            BinaryPackageCase::from(vec![("usr/share/foo/", dir())]),
        ];
        BinaryPackageTester::new(PackageContainsEmptyDirectory, pass, fail).test_and_snapshot();
    }
}
