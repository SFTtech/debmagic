use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::elf::entry_bytes;
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::super::fields_relations_more::is_transitional_or_meta;

pub struct PackageContainsNoArchDependentFiles;

declare_rule! {
    /// A Binary package whose Architecture is not `all` ships no file that
    /// depends on the architecture.
    ///
    /// Architecture dependence is shown by a directory directly under
    /// `usr/lib/<triplet>` (a `DEB_HOST_MULTIARCH` triplet), or by a regular
    /// file, hardlink, or other entry there, or by a non-empty file outside
    /// `usr/share` that is not plain ASCII text. The file type is not
    /// libmagic: a file of one byte is "very short" and a file whose bytes are
    /// all printable ASCII or the usual text controls is "ASCII text", as
    /// `file(1)` would say for them. Other non-ASCII content counts as
    /// architecture dependent, as in lintian. Skipped for udebs, Architecture
    /// `all`, and transitional or meta packages (by Description or Section).
    /// Extra and Location are empty. Severity is info, the Tag is experimental
    /// and not selected by default, as in lintian.
    PackageContainsNoArchDependentFiles,
    code = "LN1374",
    tag = "package-contains-no-arch-dependent-files",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for PackageContainsNoArchDependentFiles {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let fields = ctx.installation_control().fields().clone();
        let architecture = fields.get("Architecture").unwrap_or_default().trim();
        if architecture == "all" {
            return;
        }
        let package = ctx.package_name().to_string();
        if is_transitional_or_meta(&package, &fields) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        if !depends_on_architecture(files) {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

fn depends_on_architecture(files: &FileTree) -> bool {
    files.entries().iter().any(|entry| {
        in_triplet_dir(entry.path())
            || (entry.is_file()
                && !entry.path().starts_with("usr/share")
                && is_arch_dependent_content(files, entry))
    })
}

/// A non-empty file whose content is not plain ASCII text, as libmagic
/// classifies it. Unreadable content does not count.
fn is_arch_dependent_content(files: &FileTree, entry: &crate::lint::file_tree::Entry) -> bool {
    let Some(bytes) = entry_bytes(files, entry) else {
        return false;
    };
    if bytes.is_empty() || bytes.len() < 2 {
        return false;
    }
    !bytes
        .iter()
        .all(|&byte| matches!(byte, 0x08 | 0x09..=0x0d | 0x1b | 0x20..=0x7e))
}

/// The entry sits directly in `usr/lib/<triplet>`, lintian's
/// `dirname =~ m{^(?:usr)?/lib/([^/]+)/$}` with a known triplet.
fn in_triplet_dir(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(rest) = parent.as_os_str().as_bytes().strip_prefix(b"usr/lib/") else {
        return false;
    };
    std::str::from_utf8(rest)
        .is_ok_and(|triplet| !triplet.contains('/') && TRIPLETS.contains_key(triplet))
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

/// The `DEB_HOST_MULTIARCH` triplets, lintian's `TRIPLETS` keys.
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn text(path: &str) -> (&str, TestFile) {
        (path, TestFile::new("plain text\n"))
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Architecture all is skipped.
            BinaryPackageCase::from(vec![text("usr/share/doc/foo/README")])
                .field("Architecture", "all"),
            // Architecture dependent content.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo",
                TestFile::from_bytes(vec![0x7f, b'E', b'L', b'F', 2, 1]),
            )])
            .field("Architecture", "amd64"),
            // A file directly in usr/lib/<triplet>.
            BinaryPackageCase::from(vec![text("usr/lib/x86_64-linux-gnu/libfoo.txt")])
                .field("Architecture", "amd64"),
            // A udeb is skipped.
            BinaryPackageCase::from(vec![text("usr/bin/foo")])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        let fail = vec![
            // Empty content only.
            BinaryPackageCase::from(vec![("usr/share/foo/empty", TestFile::new(""))])
                .field("Architecture", "amd64"),
            // Plain text outside usr/share.
            BinaryPackageCase::from(vec![text("usr/bin/foo")]).field("Architecture", "amd64"),
            // Only text outside usr/share and a directory in usr/lib/<triplet>.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/", TestFile::directory())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![text("usr/share/doc/foo/README"), text("etc/foo.conf")])
                .field("Architecture", "amd64"),
        ];
        BinaryPackageTester::new(PackageContainsNoArchDependentFiles, pass, fail)
            .test_and_snapshot();
    }
}
