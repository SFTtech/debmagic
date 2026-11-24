use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsIntoObsoleteDir;

declare_rule! {
    /// An installed path matches one of lintian's obsolete directories.
    ///
    /// Walks every Data files entry against the lintian 2.139.0
    /// `data/files/obsolete-paths` table. A directory is matched with its
    /// trailing slash. Extra is `: <old> -> <new>` and the table's note; the
    /// path is the Location. Udebs are checked too.
    PackageInstallsIntoObsoleteDir,
    code = "LN0824",
    tag = "package-installs-into-obsolete-dir",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `data/files/obsolete-paths`: the match, the newer
/// directory, and the note.
const OBSOLETE_PATHS: &[(&str, &str, &str)] = &[
    (
        "^etc/bash_completion.d/",
        "usr/share/bash-completion/completions",
        "Ensure new filename matches stricter requirements (see https://bugs.debian.org/776954 and https://bugs.debian.org/814599)",
    ),
    (
        "^etc/dhcp3/",
        "etc/dhcp",
        "(see also https://bugs.debian.org/673029)",
    ),
    (
        "^etc/X11/fonts/X11R7/",
        "etc/X11/fonts",
        "(see also https://bugs.debian.org/364530#24)",
    ),
];

type Row = (&'static str, &'static str, &'static str);

static OBSOLETE: LazyLock<Vec<(Regex, &'static Row)>> = LazyLock::new(|| {
    OBSOLETE_PATHS
        .iter()
        .map(|row| (Regex::new(row.0).expect("valid regex"), row))
        .collect()
});

impl BinaryPackageRule for PackageInstallsIntoObsoleteDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let name = lintian_name(entry)?;
                let &(olddir, newdir, note) = obsolete_for(&name)?;
                Some((
                    PathBuf::from(name),
                    format!(": {olddir} -> {newdir} {note}"),
                ))
            })
            .collect();
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The path of an Entry as lintian names an installed item: a directory has a
/// trailing slash. Non-UTF-8 paths are skipped.
fn lintian_name(entry: &Entry) -> Option<String> {
    let path = entry.path().to_str()?;
    Some(if matches!(entry.kind(), EntryKind::Directory) {
        format!("{path}/")
    } else {
        path.to_string()
    })
}

/// The first table row whose match names `name`.
fn obsolete_for(name: &str) -> Option<&'static Row> {
    OBSOLETE
        .iter()
        .find(|(pattern, _)| pattern.is_match(name))
        .map(|(_, row)| *row)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("etc/dhcp", TestFile::new("x")),
            data("etc/dhcp/dhclient.conf", TestFile::new("x")),
            data("etc/dhcp3", TestFile::new("x")),
            data("etc/bash_completion", TestFile::new("x")),
            data("etc/bash_completion.d", TestFile::new("x")),
            // The match is the whole prefix, so a file of that name is not a
            // directory.
            data("etc/X11/fonts/X11R7", TestFile::new("x")),
            data("usr/etc/dhcp3/bar", TestFile::new("x")),
            data("etc/dhcp3x/bar", TestFile::new("x")),
            data("etc/dhcp", TestFile::directory()),
        ];
        let fail = vec![
            data("etc/dhcp3/bar", TestFile::new("x")),
            data("etc/dhcp3", TestFile::directory()),
            data("etc/bash_completion.d/foo", TestFile::new("x")),
            data("etc/X11/fonts/X11R7/foo", TestFile::new("x")),
            data("etc/X11/fonts/X11R7", TestFile::directory()),
            data("etc/dhcp3/bar", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(PackageInstallsIntoObsoleteDir, pass, fail).test_and_snapshot();
    }
}
