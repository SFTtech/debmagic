use std::collections::HashMap;
use std::path::{Path, PathBuf};

use debmagic_common::debian::source::SourceFormat;
use sha2::{Digest, Sha256};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct NoDebianChanges;

declare_rule! {
    /// The non-native Source package makes no changes to the upstream sources
    /// in the Debian-related files.
    ///
    /// Compares the Patched files with the Orig files. Nothing is reported
    /// when some Patched path is absent from the Orig files (an addition), or
    /// when a path in both differs in content (a file in one and not the
    /// other counts as a difference). Each Patched path is compared with its
    /// Orig counterpart by SHA-256 of its bytes, which stands for lintian's
    /// md5sum: only regular files and hardlinks have a content digest, so a
    /// directory or symlink in both trees counts as unchanged. Under
    /// `3.0 (quilt)`, additions under `.pc/` do not count. Empty Orig files
    /// (a native Source package, or a Source tree) are never reported: lintian
    /// does not report native packages, and this Rule takes an empty Orig tree
    /// as native rather than parsing the Dsc format. The Tag is a warning with
    /// empty extra.
    NoDebianChanges,
    code = "LN0926",
    tag = "no-debian-changes",
    default_selected = true,
    experimental = false,
}

/// Content digest of a regular file or hardlink; `None` for other kinds.
type Digest256 = Option<Vec<u8>>;

impl SourceRule for NoDebianChanges {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let orig: HashMap<PathBuf, Digest256> = match ctx.orig_files() {
            Some(tree) => digests(tree).into_iter().collect(),
            None => HashMap::new(),
        };
        // Empty Orig files: a native package (or a Source tree), nothing to compare.
        if orig.is_empty() {
            return;
        }
        let quilt = is_quilt(ctx);
        let Some(tree) = ctx.files() else {
            return;
        };
        let patched = digests(tree);

        let mut added = false;
        let mut touched = false;
        for (path, digest) in &patched {
            match orig.get(path) {
                None => {
                    if quilt && path.starts_with(".pc") {
                        continue;
                    }
                    added = true;
                }
                Some(orig_digest) => touched |= digest != orig_digest,
            }
        }
        if !added && !touched {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// Every entry of the tree with its content digest, in tree order.
fn digests(tree: &FileTree) -> Vec<(PathBuf, Digest256)> {
    tree.entries()
        .iter()
        .map(|entry| {
            let digest = matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                .then(|| tree.read(entry).ok())
                .flatten()
                .map(|bytes| Sha256::digest(&bytes).to_vec());
            (entry.path().to_path_buf(), digest)
        })
        .collect()
}

/// Dsc `Format` when that field is present, otherwise `debian/source/format`.
fn is_quilt(ctx: &mut SourceContext<'_>) -> bool {
    if let Some(dsc) = ctx.dsc()
        && let Some(format) = dsc.fields().get("Format")
    {
        return SourceFormat::parse(format.trim()) == SourceFormat::Quilt;
    }
    let Some(files) = ctx.files() else {
        return false;
    };
    let Some(entry) = files.get(Path::new("debian/source/format")) else {
        return false;
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    SourceFormat::parse(text.trim()) == SourceFormat::Quilt
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester};

    use super::super::testing::{added, dir, file, in_orig, quilt, symlink, text};
    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            // Debian added a file.
            added(vec![file("debian/rules")]),
            // Debian added a symlink: any added path counts, whatever its kind.
            added(vec![symlink("debian/link")]),
            // A file present in both trees with different content.
            SourcePackageCase::from(vec![text("README", "changed")])
                .orig(vec![text("README", "original")]),
            // Under quilt, `.pc/` is skipped, but a real file still counts.
            quilt(vec![
                dir(".pc"),
                file(".pc/applied-patches"),
                file("debian/rules"),
            ]),
        ];
        let fail = vec![
            // Identical trees.
            SourcePackageCase::from(vec![text("README", "same")])
                .orig(vec![text("README", "same")]),
            in_orig(vec![]),
            // Under quilt the only addition is `.pc/`.
            quilt(vec![dir(".pc"), file(".pc/applied-patches")]),
        ];
        SourcePackageTester::new(NoDebianChanges, pass, fail).test_and_snapshot();
    }
}
