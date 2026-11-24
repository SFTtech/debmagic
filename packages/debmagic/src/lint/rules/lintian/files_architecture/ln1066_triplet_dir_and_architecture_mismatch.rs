use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::Deserialize;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct TripletDirAndArchitectureMismatch;

declare_rule! {
    /// A directory `lib/<triplet>/` or `usr/lib/<triplet>/` whose triplet is the
    /// `DEB_HOST_MULTIARCH` of another release architecture than the Binary
    /// package's Architecture. Reported as `is for <arch> instead of <Architecture>`
    /// at the directory. Only directory entries directly under `lib` or
    /// `usr/lib` count; deeper directories and files do not. An `all` package
    /// with a multiarch directory is a hit too, as in lintian.
    TripletDirAndArchitectureMismatch,
    code = "LN1066",
    tag = "triplet-dir-and-architecture-mismatch",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TripletDirAndArchitectureMismatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let port = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .unwrap_or_default()
            .trim()
            .to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .query()
            .kinds([EntryKind::Directory])
            .into_iter()
            .filter_map(|entry| {
                let triplet = triplet_dir(entry.path())?;
                let from = TRIPLETS.get(triplet)?;
                (*from != port).then(|| {
                    (
                        entry.path().to_path_buf(),
                        format!("is for {from} instead of {port}"),
                    )
                })
            })
            .collect();
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[derive(Deserialize)]
struct HostFile {
    host_variables: HashMap<String, HostArch>,
}

#[derive(Deserialize)]
struct HostArch {
    #[serde(rename = "DEB_HOST_MULTIARCH")]
    multiarch: String,
}

/// `DEB_HOST_MULTIARCH` triplet to Debian architecture, lintian's `TRIPLETS`.
static TRIPLETS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let file: HostFile =
        serde_json::from_str(include_str!("../../../data/architectures/host.json"))
            .expect("vendored architectures/host.json");
    file.host_variables
        .into_iter()
        .filter(|(_, arch)| !arch.multiarch.is_empty())
        .map(|(arch, host)| (host.multiarch, arch))
        .collect()
});

/// The `<triplet>` of a path `lib/<triplet>` or `usr/lib/<triplet>`, compared
/// on raw bytes. `None` for anything else, or a triplet that is not UTF-8.
fn triplet_dir(path: &Path) -> Option<&str> {
    let bytes = path.as_os_str().as_bytes();
    let rest = bytes.strip_prefix(b"usr/").unwrap_or(bytes);
    let name = rest.strip_prefix(b"lib/")?;
    // Some paths keep the trailing slash lintian's names have.
    let name = name.strip_suffix(b"/").unwrap_or(name);
    if name.is_empty() || name.contains(&b'/') {
        return None;
    }
    std::str::from_utf8(name).ok()
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Matching triplet.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/", dir())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("lib/i386-linux-gnu/", dir())])
                .field("Architecture", "i386"),
            // Not a triplet directory.
            BinaryPackageCase::from(vec![("usr/lib/foo/", dir())]).field("Architecture", "amd64"),
            // Deeper than the triplet directory.
            BinaryPackageCase::from(vec![("usr/lib/i386-linux-gnu/sub/", dir())])
                .field("Architecture", "amd64"),
            // A file, not a directory.
            BinaryPackageCase::from(vec![("usr/lib/i386-linux-gnu", TestFile::new("not a dir"))])
                .field("Architecture", "amd64"),
            // Not under lib.
            BinaryPackageCase::from(vec![("usr/share/i386-linux-gnu/", dir())])
                .field("Architecture", "amd64"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/i386-linux-gnu/", dir())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/", dir())])
                .field("Architecture", "i386"),
            // Without the usr/ prefix.
            BinaryPackageCase::from(vec![("lib/i386-linux-gnu/", dir())])
                .field("Architecture", "amd64"),
            // An all package is not a match for any triplet.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/", dir())])
                .field("Architecture", "all"),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("usr/lib/i386-linux-gnu/", dir())])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(TripletDirAndArchitectureMismatch, pass, fail).test_and_snapshot();
    }
}
