//! The changelog files in a Binary package's doc directory, as lintian's
//! `debian/changelog` Check judges them (`no-changelog`,
//! `debian-changelog-file-missing-or-wrong-name`).

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::{EntryKind, FileTree};

/// Which changelog a package lacks, when lintian says it lacks one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MissingChangelog {
    /// A native package without `changelog` / `changelog.gz`.
    Native,
    /// A foreign non-native package with neither a Debian nor an upstream text changelog.
    Debian,
    /// A foreign non-native package with only an upstream text changelog.
    DebianButUpstreamText,
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

/// Judge the Binary package's changelog files. `None` when nothing is
/// missing, for udebs, for a symlinked doc directory, and without Data files.
pub(super) fn missing_changelog(ctx: &mut BinaryPackageContext<'_>) -> Option<MissingChangelog> {
    if ctx.is_udeb() {
        return None;
    }
    let package = ctx.package_name().to_string();
    let fields = ctx.installation_control().fields();
    let native = is_native(fields.get("Source"), fields.get("Version"));
    let version = fields
        .get("Version")
        .filter(|version| !version.is_empty())
        .unwrap_or("0-1")
        .to_string();
    classify(ctx.data_files()?, &package, native, &version)
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

fn classify(
    files: &FileTree,
    package: &str,
    native: bool,
    version: &str,
) -> Option<MissingChangelog> {
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
        return Some(MissingChangelog::Native);
    }
    if any(&DEBIAN_CHANGELOG) || any(&DEBIAN_WRONG_NAME) {
        return None;
    }
    // A package with a `-0.x` version could be native or not; lintian says nothing.
    if version.contains("-0.") {
        return None;
    }
    Some(if any(&NATIVE_CHANGELOG) {
        MissingChangelog::DebianButUpstreamText
    } else {
        MissingChangelog::Debian
    })
}
