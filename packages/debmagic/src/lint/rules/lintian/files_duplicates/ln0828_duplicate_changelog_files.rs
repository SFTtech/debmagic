use std::collections::BTreeMap;

use debmagic_common::checksums::Hash;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct DuplicateChangelogFiles;

declare_rule! {
    /// Two or more identical non-empty files under `usr/share/doc/` include a
    /// changelog.
    ///
    /// Regular Data files under `usr/share/doc/` are grouped by their md5.
    /// A group of at least two files is flagged when one of its paths contains
    /// `changelog` in any case. Each flagged group is reported once, in
    /// checksum order, and its paths are sorted and joined by spaces as the
    /// Extra. No Location. Udebs are checked too.
    DuplicateChangelogFiles,
    code = "LN0828",
    tag = "duplicate-changelog-files",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateChangelogFiles {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for names in duplicates(ctx) {
            ctx.diagnostic(Diagnostic::warn(names.join(" ")));
        }
    }
}

/// Sorted paths of each flagged group, in checksum order.
fn duplicates(ctx: &mut BinaryPackageContext<'_>) -> Vec<Vec<String>> {
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in files.query().kinds([EntryKind::RegularFile]) {
        if !entry.path().starts_with("usr/share/doc") {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let name = entry.path().to_string_lossy().into_owned();
        groups.entry(Hash::Md5.hex(&bytes)).or_default().push(name);
    }
    groups
        .into_values()
        .filter(|names| names.len() > 1 && names.iter().any(|name| is_changelog(name)))
        .map(|mut names| {
            names.sort();
            names
        })
        .collect()
}

fn is_changelog(name: &str) -> bool {
    name.to_ascii_lowercase().contains("changelog")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Duplicates without a changelog in their paths.
            files(&[
                ("usr/share/doc/example/NEWS.gz", "same"),
                ("usr/share/doc/example/COPYING", "same"),
            ]),
            // A changelog with unique content.
            files(&[
                ("usr/share/doc/example/changelog.gz", "one"),
                ("usr/share/doc/example/NEWS.gz", "two"),
            ]),
            // Empty files are never duplicates.
            files(&[
                ("usr/share/doc/example/changelog.gz", ""),
                ("usr/share/doc/example/NEWS.gz", ""),
            ]),
            // Only usr/share/doc is checked.
            files(&[
                ("usr/share/man/man1/changelog.gz", "same"),
                ("usr/share/info/changelog.gz", "same"),
            ]),
        ];
        let fail = vec![
            files(&[
                ("usr/share/doc/example/NEWS.gz", "same"),
                ("usr/share/doc/example/changelog.gz", "same"),
            ]),
            // Any case of changelog counts, and every duplicate is listed.
            files(&[
                ("usr/share/doc/example/COPYING", "same"),
                ("usr/share/doc/other/ChangeLog", "same"),
                ("usr/share/doc/example/NEWS", "same"),
            ]),
            files(&[
                ("usr/share/doc/example/NEWS.gz", "same"),
                ("usr/share/doc/example/changelog.gz", "same"),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(DuplicateChangelogFiles, pass, fail).test_and_snapshot();
    }
}
