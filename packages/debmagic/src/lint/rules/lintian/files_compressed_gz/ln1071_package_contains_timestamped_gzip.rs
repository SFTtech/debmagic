use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use chrono::DateTime;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::gzip::gunzip;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsTimestampedGzip;

declare_rule! {
    /// A gzip-compressed file records a modification time after the newest
    /// installed changelog entry, so the package was not built reproducibly
    /// (see Debian bug 762105).
    ///
    /// lintian reads the gzip header time (bytes 4 to 8, little-endian) of
    /// every regular file and hardlink whose content starts with the gzip
    /// magic, and skips a zero time. The newest entry's timestamp of the
    /// installed changelog (`usr/share/doc/<pkg>/changelog.Debian.gz`, or the
    /// other names lintian tries) is the reference, `0` when there is none.
    /// A time later than that is reported with the UTC time as the extra, unless
    /// the package is Multi-Arch `same` and the path does not name its
    /// Architecture, which is `gzip-file-is-not-multi-arch-same-safe`
    /// instead. Extra is the time `YYYY-MM-DDTHH:MM:SS`; the path is the Location.
    ///
    /// Differs from lintian: the file type is sniffed from the gzip magic
    /// instead of `file(1)`, and the changelog is read in process. An entry
    /// whose bytes cannot be read is skipped.
    PackageContainsTimestampedGzip,
    code = "LN1071",
    tag = "package-contains-timestamped-gzip",
    default_selected = true,
    experimental = false,
}

/// lintian's `$GZIP_HEADER_SIZE`.
const GZIP_HEADER_SIZE: usize = 8;

/// The names lintian tries for the installed changelog, in order.
const CHANGELOG_NAMES: &[&str] = &[
    "changelog.Debian.gz",
    "changelog.Debian",
    "changelog.debian.gz",
    "changelog.debian",
    "changelog.gz",
    "changelog",
];

/// The Binary package fields the gzip Checks read.
pub(super) struct BinaryFacts {
    package: String,
    architecture: Option<String>,
    multi_arch: String,
}

/// Which of the two gzip Tags a timestamped file falls under.
pub(super) enum GzipTimestamp {
    /// `gzip-file-is-not-multi-arch-same-safe`.
    MultiArchSameUnsafe,
    /// `package-contains-timestamped-gzip`, with the header time as UTC.
    Timestamped(String),
}

pub(super) fn binary_facts(ctx: &BinaryPackageContext<'_>) -> BinaryFacts {
    let fields = ctx.installation_control().fields();
    BinaryFacts {
        package: ctx.package_name().to_string(),
        architecture: fields.unfolded_value("Architecture"),
        // Perl `value('Multi-Arch') || 'no'`.
        multi_arch: fields
            .unfolded_value("Multi-Arch")
            .filter(|value| !value.is_empty() && value != "0")
            .unwrap_or_else(|| "no".to_string()),
    }
}

/// Every gzip file of the Data files that lintian's timestamp Checks look at,
/// in file-tree order, with the Tag that applies to it.
pub(super) fn gzip_findings(
    files: &FileTree,
    facts: &BinaryFacts,
) -> Vec<(PathBuf, GzipTimestamp)> {
    let changelog = changelog_timestamp(files, &facts.package);
    files
        .entries()
        .iter()
        .filter(|entry| entry.is_file())
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let finding = gzip_timestamp(facts, changelog, entry.path(), &bytes)?;
            Some((entry.path().to_path_buf(), finding))
        })
        .collect()
}

/// `changelog_timestamp`: the newest installed changelog entry's timestamp,
/// `0` when there is no installed changelog or it has no timestamp.
fn changelog_timestamp(files: &FileTree, package: &str) -> i64 {
    let Some(entry) = installed_changelog(files, package) else {
        return 0;
    };
    let Some(text) = changelog_text(files, entry) else {
        return 0;
    };
    DebianChangelog::parse(&text)
        .entries()
        .first()
        .and_then(|entry| entry.timestamp())
        .unwrap_or(0)
}

fn installed_changelog<'a>(files: &'a FileTree, package: &str) -> Option<&'a Entry> {
    let dir = format!("usr/share/doc/{package}");
    CHANGELOG_NAMES.iter().find_map(|name| {
        let entry = files.get(format!("{dir}/{name}"))?;
        matches!(
            entry.kind(),
            EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
        )
        .then_some(entry)
    })
}

fn changelog_text(files: &FileTree, entry: &Entry) -> Option<String> {
    let resolved = resolve(files, entry)?;
    if !matches!(
        resolved.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink
    ) {
        return None;
    }
    let bytes = files.read(resolved).ok()?;
    let plain = if resolved
        .path()
        .to_str()
        .is_some_and(|path| path.ends_with(".gz"))
    {
        gunzip(&bytes)?
    } else {
        bytes
    };
    String::from_utf8(plain).ok()
}

fn resolve<'a>(files: &'a FileTree, entry: &'a Entry) -> Option<&'a Entry> {
    let mut current = entry;
    for _ in 0..8 {
        if current.kind() != EntryKind::Symlink {
            return Some(current);
        }
        let next = resolve_link(current.path(), current.link_target()?)?;
        current = files.get(&next)?;
    }
    None
}

fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

/// lintian's branch in `visit_installed_files` for one file. `None` when the
/// file is not gzip, its header time is `0`, or it is not after the changelog.
fn gzip_timestamp(
    facts: &BinaryFacts,
    changelog: i64,
    path: &Path,
    bytes: &[u8],
) -> Option<GzipTimestamp> {
    if !bytes.starts_with(&[0x1f, 0x8b]) || bytes.len() < GZIP_HEADER_SIZE {
        return None;
    }
    let gziptime = u32::from_le_bytes(bytes[4..GZIP_HEADER_SIZE].try_into().ok()?);
    if gziptime == 0 || i64::from(gziptime) - changelog <= 0 {
        return None;
    }
    // Perl `$item->name !~ /\Q$architecture\E/`: an undefined Architecture
    // interpolates as an empty pattern, which always matches.
    let same_arch_missing = facts.multi_arch == "same"
        && facts.architecture.as_deref().is_some_and(|architecture| {
            !contains(path.as_os_str().as_bytes(), architecture.as_bytes())
        });
    if same_arch_missing {
        return Some(GzipTimestamp::MultiArchSameUnsafe);
    }
    let when = DateTime::from_timestamp(i64::from(gziptime), 0)?;
    Some(GzipTimestamp::Timestamped(
        when.format("%Y-%m-%dT%H:%M:%S").to_string(),
    ))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

impl BinaryPackageRule for PackageContainsTimestampedGzip {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = binary_facts(ctx);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let findings: Vec<(PathBuf, String)> = gzip_findings(files, &facts)
            .into_iter()
            .filter_map(|(path, finding)| match finding {
                GzipTimestamp::Timestamped(time) => Some((path, time)),
                GzipTimestamp::MultiArchSameUnsafe => None,
            })
            .collect();
        for (path, time) in findings {
            ctx.diagnostic(Diagnostic::warn(time).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(plain: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain).expect("write");
        encoder.finish().expect("finish")
    }

    /// A gzip header with `mtime`, then some bytes that are not a valid
    /// deflate stream: the Rule reads the header only.
    fn gzip_with_time(mtime: u32) -> Vec<u8> {
        let mut bytes = vec![0x1f, 0x8b, 0x08, 0x00];
        bytes.extend(mtime.to_le_bytes());
        bytes.extend([0x00, 0x03, b'j', b'u', b'n', b'k']);
        bytes
    }

    /// 2020-03-02T23:44:19 UTC.
    const TIME: u32 = 1_583_192_659;
    /// 2020-01-01T00:00:00 UTC, the changelog's newest entry.
    const CHANGELOG: &str = "example (1.0) unstable; urgency=low\n\n  * test\n\n -- A <a@example.org>  Wed, 01 Jan 2020 00:00:00 +0000\n";
    /// 2021-01-01T00:00:00 UTC, after the header time.
    const CHANGELOG_2021: &str = "example (2.0) unstable; urgency=low\n\n  * test\n\n -- A <a@example.org>  Fri, 01 Jan 2021 00:00:00 +0000\n";

    fn with_changelog(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        let mut all: Vec<(&str, TestFile)> = vec![(
            "usr/share/doc/example/changelog.Debian",
            TestFile::new(CHANGELOG),
        )];
        all.extend(files);
        BinaryPackageCase::from(all)
    }

    #[test]
    fn binary() {
        let timed = || TestFile::from_bytes(gzip_with_time(TIME));
        let pass = vec![
            // No gzip files.
            with_changelog(vec![("etc/plain", TestFile::new("plain"))]),
            // Header time of zero is skipped.
            with_changelog(vec![(
                "etc/zero.gz",
                TestFile::from_bytes(gzip_with_time(0)),
            )]),
            // Header time not after the changelog.
            with_changelog(vec![(
                "etc/early.gz",
                TestFile::from_bytes(gzip_with_time(1_577_836_800)),
            )]),
            // Gzip magic but too short for the header time.
            with_changelog(vec![(
                "etc/short.gz",
                TestFile::from_bytes(vec![0x1f, 0x8b, 0x08]),
            )]),
            // No magic: not gzip, even with a header-shaped tail.
            with_changelog(vec![(
                "etc/fake.gz",
                TestFile::from_bytes(b"not-gzip!!!!".to_vec()),
            )]),
            // Multi-Arch same, path without the Architecture: the other Tag.
            with_changelog(vec![("etc/after-changelog.gz", timed())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // The installed changelog is read from its gzip form: its newest
            // entry is after the header time, so nothing is timestamped.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/changelog.Debian.gz",
                    TestFile::from_bytes(gzip(CHANGELOG_2021.as_bytes())),
                ),
                ("etc/after-changelog.gz", timed()),
            ]),
        ];
        let fail = vec![
            with_changelog(vec![("etc/after-changelog.gz", timed())]),
            // Multi-Arch no.
            with_changelog(vec![("etc/after-changelog.gz", timed())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "no"),
            // Multi-Arch same but the path names the Architecture: this Tag.
            with_changelog(vec![("usr/lib/amd64/x.gz", timed())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // No Architecture field: the pattern is empty and always matches.
            with_changelog(vec![("etc/after-changelog.gz", timed())]).field("Multi-Arch", "same"),
            // Hardlink to a timestamped gzip.
            with_changelog(vec![
                ("etc/target.gz", timed()),
                ("etc/link.gz", timed().hardlink_to("etc/target.gz")),
            ]),
            // Non-UTF-8 name bytes.
            BinaryPackageCase::from(vec![
                (
                    OsString::from_vec(b"usr/share/doc/example/changelog.Debian".to_vec()),
                    TestFile::new(CHANGELOG),
                ),
                (OsString::from_vec(b"etc/\xff.gz".to_vec()), timed()),
            ]),
            // A udeb is checked like a deb.
            with_changelog(vec![("etc/after-changelog.gz", timed())]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsTimestampedGzip, pass, fail).test_and_snapshot();
    }
}
