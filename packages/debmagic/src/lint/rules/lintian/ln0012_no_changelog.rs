use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::changelog_doc::{MissingChangelog, missing_changelog};

pub struct NoChangelog;

declare_rule! {
    /// The Binary package ships no changelog in its doc directory.
    ///
    /// Looks at the files and symlinks directly in `usr/share/doc/<pkg>`; a
    /// symlinked doc directory is judged by other Tags. A native package
    /// (the version in `Source`, else `Version`, has no `-`) needs
    /// `changelog` or `changelog.gz`; the extra is
    /// `usr/share/doc/<pkg>/changelog.gz (native package)`. A non-native
    /// package needs `changelog.Debian` or `changelog.Debian.gz`; the extra
    /// is `usr/share/doc/<pkg>/changelog.Debian.gz (non-native package)`, and
    /// only for a foreign package (the `Version` has no `-0.`) that has no
    /// upstream text changelog either. Wrongly cased or named files are other
    /// Tags. Udebs are not checked. Location is empty.
    NoChangelog,
    code = "LN0012",
    tag = "no-changelog",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let doc_dir = format!("usr/share/doc/{}", ctx.package_name());
        let extra = match missing_changelog(ctx) {
            Some(MissingChangelog::Native) => format!("{doc_dir}/changelog.gz (native package)"),
            Some(MissingChangelog::Debian) => {
                format!("{doc_dir}/changelog.Debian.gz (non-native package)")
            }
            Some(MissingChangelog::DebianButUpstreamText) | None => return,
        };
        ctx.diagnostic(Diagnostic::error(extra));
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

    fn native(case: BinaryPackageCase) -> BinaryPackageCase {
        case.field("Version", "1.0")
    }

    #[test]
    fn test() {
        let pass = vec![
            // Non-native with the Debian changelog.
            files(&["usr/share/doc/example/changelog.Debian.gz"]).field("Version", "1.0-1"),
            files(&["usr/share/doc/example/changelog.Debian"]).field("Version", "1.0-1"),
            // Native with its changelog.
            native(files(&["usr/share/doc/example/changelog.gz"])),
            native(files(&["usr/share/doc/example/changelog"])),
            // A symlink counts as a file.
            native(BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.gz",
                TestFile::symlink("../other/changelog.gz"),
            )])),
            // Wrong names are other Tags.
            files(&["usr/share/doc/example/changelog.debian.gz"]).field("Version", "1.0-1"),
            native(files(&["usr/share/doc/example/changelog.debian.gz"])),
            // Case does not matter for that wrong name.
            native(files(&["usr/share/doc/example/changelog.Debian.gz"])),
            // An upstream text changelog leaves it to another Tag.
            files(&["usr/share/doc/example/changelog.gz"]).field("Version", "1.0-1"),
            // A `-0.x` version is not known to be foreign.
            files(&["usr/share/doc/example/copyright"]).field("Version", "1.0-0.1"),
            // A symlinked doc directory is judged elsewhere.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::symlink("other"))])
                .field("Version", "1.0-1"),
            // Udebs carry no changelog.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Version", "1.0-1")
                .udeb(),
            // The version in Source decides nativeness.
            files(&["usr/share/doc/example/changelog.gz"])
                .field("Source", "other (1.0)")
                .field("Version", "1.0-1"),
            files(&["usr/share/doc/example/changelog.Debian.gz"])
                .field("Source", "other (1.0-1)")
                .field("Version", "1.0"),
        ];
        let fail = vec![
            // Nothing at all.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Version", "1.0-1"),
            native(BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())),
            // Doc directory without a changelog.
            files(&["usr/share/doc/example/copyright"]).field("Version", "1.0-1"),
            native(files(&["usr/share/doc/example/copyright"])),
            // An HTML-only upstream changelog is not a text changelog.
            files(&["usr/share/doc/example/changelog.html.gz"]).field("Version", "1.0-1"),
            // A native package needs the plain name; this one is not `.gz` either.
            native(files(&["usr/share/doc/example/changelog.debian"])),
            // Another package's changelog does not count.
            files(&["usr/share/doc/other/changelog.Debian.gz"]).field("Version", "1.0-1"),
            // The changelog must sit directly in the doc directory.
            files(&["usr/share/doc/example/sub/changelog.Debian.gz"]).field("Version", "1.0-1"),
            // A directory is not a changelog.
            native(BinaryPackageCase::from(vec![(
                "usr/share/doc/example/changelog.gz",
                TestFile::directory(),
            )])),
            // Without a Version the package is foreign and non-native.
            files(&["usr/share/doc/example/copyright"]),
            // A hyphen in Source's version makes it non-native despite Version.
            files(&["usr/share/doc/example/copyright"])
                .field("Source", "other (1.0-1)")
                .field("Version", "1.0"),
        ];
        BinaryPackageTester::new(NoChangelog, pass, fail).test_and_snapshot();
    }
}
