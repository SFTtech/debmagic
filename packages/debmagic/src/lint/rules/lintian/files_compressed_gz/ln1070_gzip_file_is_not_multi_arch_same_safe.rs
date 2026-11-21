use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln1071_package_contains_timestamped_gzip::{GzipTimestamp, binary_facts, gzip_findings};

pub struct GzipFileIsNotMultiArchSameSafe;

declare_rule! {
    /// A gzip file has a header time after the newest installed changelog
    /// entry, in a package that is Multi-Arch `same`, and its path does not
    /// name the Architecture. Such a file differs between architectures, so
    /// `same` is unsafe.
    ///
    /// The Multi-Arch and Architecture fields are read from the installation
    /// control, `no` when Multi-Arch is missing. A path containing the
    /// Architecture bytes is not reported here; it is reported by
    /// `package-contains-timestamped-gzip` instead. The gzip detection, the
    /// changelog reference and the header time are those of that Tag. Extra
    /// is empty; the path is the Location.
    GzipFileIsNotMultiArchSameSafe,
    code = "LN1070",
    tag = "gzip-file-is-not-multi-arch-same-safe",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GzipFileIsNotMultiArchSameSafe {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = binary_facts(ctx);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = gzip_findings(files, &facts)
            .into_iter()
            .filter_map(|(path, finding)| match finding {
                GzipTimestamp::MultiArchSameUnsafe => Some(path),
                GzipTimestamp::Timestamped(_) => None,
            })
            .collect();
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
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// A gzip header with `mtime`, then bytes that are not a valid deflate
    /// stream: the Rule reads the header only.
    fn gzip_with_time(mtime: u32) -> Vec<u8> {
        let mut bytes = vec![0x1f, 0x8b, 0x08, 0x00];
        bytes.extend(mtime.to_le_bytes());
        bytes.extend([0x00, 0x03, b'j', b'u', b'n', b'k']);
        bytes
    }

    fn gzip(plain: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain).expect("write");
        encoder.finish().expect("finish")
    }

    /// 2020-03-02T23:44:19 UTC.
    const TIME: u32 = 1_583_192_659;
    /// 2020-01-01T00:00:00 UTC, the changelog's newest entry.
    const CHANGELOG: &str = "example (1.0) unstable; urgency=low\n\n  * test\n\n -- A <a@example.org>  Wed, 01 Jan 2020 00:00:00 +0000\n";

    fn same(path: &str) -> BinaryPackageCase {
        with_changelog(vec![(path, TestFile::from_bytes(gzip_with_time(TIME)))])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same")
    }

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
        let pass = vec![
            // Multi-Arch no or missing.
            with_changelog(vec![(
                "etc/after-changelog.gz",
                TestFile::from_bytes(gzip_with_time(TIME)),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "no"),
            with_changelog(vec![(
                "etc/after-changelog.gz",
                TestFile::from_bytes(gzip_with_time(TIME)),
            )])
            .field("Architecture", "amd64"),
            // Multi-Arch same, and the path names the Architecture.
            same("usr/lib/amd64/x.gz"),
            // Header time not after the changelog, or zero.
            with_changelog(vec![(
                "etc/early.gz",
                TestFile::from_bytes(gzip_with_time(1_577_836_800)),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            with_changelog(vec![(
                "etc/zero.gz",
                TestFile::from_bytes(gzip_with_time(0)),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // Not gzip.
            with_changelog(vec![("etc/plain", TestFile::new("plain"))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // No Architecture field: the empty pattern always matches, so the
            // path is not same-safe; it is the other Tag's.
            with_changelog(vec![(
                "etc/after-changelog.gz",
                TestFile::from_bytes(gzip_with_time(TIME)),
            )])
            .field("Multi-Arch", "same"),
            // Installed changelog read from its gzip form, newer than the file.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/doc/example/changelog.Debian.gz",
                    TestFile::from_bytes(gzip(
                        b"example (2.0) unstable; urgency=low\n\n  * test\n\n -- A <a@example.org>  Fri, 01 Jan 2021 00:00:00 +0000\n",
                    )),
                ),
                (
                    "etc/after-changelog.gz",
                    TestFile::from_bytes(gzip_with_time(TIME)),
                ),
            ])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
        ];
        let fail = vec![
            same("etc/after-changelog.gz"),
            // A Multi-Arch same path next to an Architecture-named one: only
            // the unnamed path is reported.
            with_changelog(vec![
                (
                    "etc/after-changelog.gz",
                    TestFile::from_bytes(gzip_with_time(TIME)),
                ),
                (
                    "usr/lib/amd64/x.gz",
                    TestFile::from_bytes(gzip_with_time(TIME)),
                ),
            ])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // Hardlink.
            with_changelog(vec![
                ("etc/target.gz", TestFile::from_bytes(gzip_with_time(TIME))),
                (
                    "etc/link.gz",
                    TestFile::from_bytes(gzip_with_time(TIME)).hardlink_to("etc/target.gz"),
                ),
            ])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // A udeb is checked like a deb.
            same("etc/after-changelog.gz").udeb(),
        ];
        BinaryPackageTester::new(GzipFileIsNotMultiArchSameSafe, pass, fail).test_and_snapshot();
    }
}
