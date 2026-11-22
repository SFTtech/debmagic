use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ArchDepPackageHasBigUsrShare;

declare_rule! {
    /// An architecture-dependent package ships a lot of data in `/usr/share`.
    ///
    /// Skipped for `Architecture: all`. Over the Data files regular files (not
    /// hardlinks), the sizes are summed: all of them as the package size, and
    /// those under `usr/share/` as the `/usr/share` size, both in whole kB
    /// (`sprintf('%.0f')`, which rounds half to even). The percentage is the
    /// `/usr/share` share of the package, rounded the same way. Reported when
    /// the percentage is over 50 and `/usr/share` is over 4096 kB, or when
    /// `/usr/share` is over 8192 kB. Extra is `<kB> kB <percent>%`, for example
    /// `4097kB 100%`; no Location. Severity is info and the Tag is not selected
    /// by default, as in lintian.
    ///
    /// Differences from lintian: a file's size is the length of its contents
    /// (there is no size field on an Entry), and a package with no regular file
    /// is skipped as lintian skips a zero total.
    ArchDepPackageHasBigUsrShare,
    code = "LN1565",
    tag = "arch-dep-package-has-big-usr-share",
    default_selected = false,
    experimental = false,
}

const KIB: f64 = 1024.0;
const SOFT_LIMIT_KIB: u64 = 4096;
const HARD_LIMIT_KIB: u64 = 8192;
const PERCENT_LIMIT: u64 = 50;

impl BinaryPackageRule for ArchDepPackageHasBigUsrShare {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .unwrap_or_default()
            .to_string();
        if architecture == "all" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let Some((total, usr_share)) = sizes(files) else {
            return;
        };
        let size_usr_share = round_kib(usr_share);
        let percentage = (usr_share as f64 / total as f64 * 100.0).round_ties_even() as u64;
        if (percentage > PERCENT_LIMIT && size_usr_share > SOFT_LIMIT_KIB)
            || size_usr_share > HARD_LIMIT_KIB
        {
            ctx.diagnostic(
                Diagnostic::warn(format!("{size_usr_share}kB {percentage}%"))
                    .with_severity(Severity::Info),
            );
        }
    }
}

/// The total size and the `usr/share/` size of the regular files, in bytes.
/// `None` when the total is zero.
fn sizes(files: &FileTree) -> Option<(u64, u64)> {
    let mut total = 0_u64;
    let mut usr_share = 0_u64;
    for entry in files.entries() {
        if !entry.is_regular_file() {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let length = bytes.len() as u64;
        total += length;
        if entry.path().starts_with("usr/share/") {
            usr_share += length;
        }
    }
    (total > 0).then_some((total, usr_share))
}

/// `sprintf('%.0f', bytes / 1024)`.
fn round_kib(bytes: u64) -> u64 {
    (bytes as f64 / KIB).round_ties_even() as u64
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn sized(path: &str, bytes: usize) -> (String, TestFile) {
        (path.to_string(), TestFile::from_bytes(vec![b'x'; bytes]))
    }

    fn package(architecture: &str, files: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files).field("Architecture", architecture)
    }

    #[test]
    fn binary() {
        let kib = 1024;
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Architecture", "amd64"),
            // Architecture-independent packages are skipped.
            package("all", vec![sized("usr/share/big/data", 5 * kib * kib)]),
            // Under 4096 kB of /usr/share.
            package("amd64", vec![sized("usr/share/data", 4096 * kib)]),
            // Over 4096 kB, but under 50 percent of the package.
            package(
                "amd64",
                vec![
                    sized("usr/share/data", 4097 * kib),
                    sized("usr/lib/x86_64-linux-gnu/liba.so", 9000 * kib),
                ],
            ),
            // Over 50 percent, but under 4096 kB.
            package(
                "amd64",
                vec![
                    sized("usr/share/data", 3000 * kib),
                    sized("usr/bin/tool", 1000 * kib),
                ],
            ),
            // Hardlinks and symlinks are not counted.
            package(
                "amd64",
                vec![
                    ("usr/share/link".to_string(), TestFile::symlink("data")),
                    ("usr/share/copy".to_string(), TestFile::new("x").hardlink()),
                ],
            ),
        ];
        let fail = vec![
            // Over 4096 kB and over half the package.
            package("amd64", vec![sized("usr/share/data", 4097 * kib)]),
            // Over 8192 kB, even under half the package.
            package(
                "amd64",
                vec![
                    sized("usr/share/data", 8193 * kib),
                    sized("usr/lib/x86_64-linux-gnu/liba.so", 9000 * kib),
                ],
            ),
        ];
        BinaryPackageTester::new(ArchDepPackageHasBigUsrShare, pass, fail).test_and_snapshot();
    }
}
