use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

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

static NATIVE_CHANGELOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog(?:\.gz)?$").expect("valid regex"));
static NATIVE_WRONG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^changelog\.debian\.gz$").expect("valid regex"));
static DEBIAN_CHANGELOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^changelog\.Debian(?:\.gz)?$").expect("valid regex"));
static DEBIAN_WRONG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^changelog\.debian(?:\.gz)?$").expect("valid regex"));
static SOURCE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\((.*)\)").expect("valid regex"));

impl BinaryPackageRule for NoChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let fields = ctx.installation_control().fields();
        let native = is_native(fields.get("Source"), fields.get("Version"));
        let version = fields
            .get("Version")
            .filter(|version| !version.is_empty())
            .unwrap_or("0-1")
            .to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if let Some(extra) = missing_changelog(files, &package, native, &version) {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

/// lintian: the version in `Source (version)`, else `Version`, has no hyphen.
/// Unknown counts as non-native.
fn is_native(source: Option<&str>, version: Option<&str>) -> bool {
    let source_version = source
        .and_then(|source| SOURCE_VERSION.captures(source))
        .and_then(|captures| captures.get(1))
        .map(|found| found.as_str())
        .or(version);
    source_version.is_some_and(|version| !version.contains('-'))
}

/// The extra of the hint, when the package lacks the changelog it needs.
fn missing_changelog(
    files: &FileTree,
    package: &str,
    native: bool,
    version: &str,
) -> Option<String> {
    let doc_dir = format!("usr/share/doc/{package}");
    if files
        .get(&doc_dir)
        .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
    {
        return None;
    }
    let prefix = format!("{doc_dir}/");
    let names: Vec<&str> = files
        .entries()
        .iter()
        .filter(|entry| entry.kind() != EntryKind::Directory)
        .filter_map(|entry| entry.path().to_str()?.strip_prefix(&prefix))
        .filter(|name| !name.contains('/'))
        .collect();
    let any = |pattern: &Regex| names.iter().any(|name| pattern.is_match(name));

    if native {
        if any(&NATIVE_CHANGELOG) || any(&NATIVE_WRONG_NAME) {
            return None;
        }
        return Some(format!("{prefix}changelog.gz (native package)"));
    }
    if any(&DEBIAN_CHANGELOG) || any(&DEBIAN_WRONG_NAME) {
        return None;
    }
    // A package with a `-0.x` version could be native or not; lintian says nothing.
    let foreign = !version.contains("-0.");
    // An upstream text changelog is judged by `debian-changelog-file-missing-or-wrong-name`.
    if foreign && !any(&NATIVE_CHANGELOG) {
        return Some(format!("{prefix}changelog.Debian.gz (non-native package)"));
    }
    None
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
