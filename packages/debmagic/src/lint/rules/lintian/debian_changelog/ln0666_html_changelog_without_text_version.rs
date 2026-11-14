use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::super::ln0024_changelog_file_not_compressed::doc_changelog_files;

pub struct HtmlChangelogWithoutTextVersion;

declare_rule! {
    /// An HTML changelog is installed without a plain-text changelog.
    ///
    /// Looks at the files and symlinks directly in `usr/share/doc/<pkg>`
    /// (skipped when that path is itself a symlink) named `changelog`,
    /// `changelog.html` or `changelog.Debian`, with or without `.gz`, exactly
    /// in that case. When none is `changelog` or `changelog.gz`, every
    /// `changelog.html` or `changelog.html.gz` is reported. A
    /// `changelog.Debian` does not count as a text version. Extra is empty.
    /// The Location is the file. Udebs are not checked.
    HtmlChangelogWithoutTextVersion,
    code = "LN0666",
    tag = "html-changelog-without-text-version",
    default_selected = true,
    experimental = false,
}

fn name_of(path: &std::path::Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

impl BinaryPackageRule for HtmlChangelogWithoutTextVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let changelogs = doc_changelog_files(ctx);
        if changelogs
            .iter()
            .any(|(path, _)| matches!(name_of(path), "changelog" | "changelog.gz"))
        {
            return;
        }
        for (path, _) in changelogs {
            if !matches!(name_of(&path), "changelog.html" | "changelog.html.gz") {
                continue;
            }
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&[]),
            files(&[
                "usr/share/doc/example/changelog.html.gz",
                "usr/share/doc/example/changelog.gz",
            ]),
            files(&[
                "usr/share/doc/example/changelog.html",
                "usr/share/doc/example/changelog",
            ]),
            files(&["usr/share/doc/example/changelog.Debian.gz"]),
            files(&["usr/share/doc/example/sub/changelog.html.gz"]),
            files(&["usr/share/doc/example/Changelog.html.gz"]),
            files(&["usr/share/doc/example/changelog.html.gz"]).udeb(),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::symlink("other")),
                ("usr/share/doc/other/changelog.html", TestFile::new("html")),
            ]),
        ];
        let fail = vec![
            files(&["usr/share/doc/example/changelog.html.gz"]),
            files(&["usr/share/doc/example/changelog.html"]),
            files(&[
                "usr/share/doc/example/changelog.html.gz",
                "usr/share/doc/example/changelog.Debian.gz",
            ]),
        ];
        BinaryPackageTester::new(HtmlChangelogWithoutTextVersion, pass, fail).test_and_snapshot();
    }
}
