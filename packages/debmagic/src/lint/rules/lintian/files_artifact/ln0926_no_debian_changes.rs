use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use debmagic_common::debian::source::SourceFormat;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
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
    /// Orig counterpart by its bytes, which stands for lintian's md5sum: only
    /// regular files and hardlinks have content, so a directory or symlink in
    /// both trees counts as unchanged. Under
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

impl SourceRule for NoDebianChanges {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(orig) = ctx.orig_files() else {
            return;
        };
        // Empty Orig files: a native package (or a Source tree), nothing to compare.
        if orig.entries().is_empty() {
            return;
        }
        let quilt = is_quilt(ctx);
        let Some(patched) = ctx.files() else {
            return;
        };
        // A later entry with the same path wins, as in a map built from the tree.
        let orig_entries: HashMap<&Path, &Entry> = orig
            .entries()
            .iter()
            .map(|entry| (entry.path(), entry))
            .collect();

        // An addition reports nothing, whatever the content, so the first
        // one ends the Rule before any bytes are looked at.
        let mut pairs = Vec::new();
        for entry in patched.entries() {
            match orig_entries.get(entry.path()) {
                Some(orig_entry) => pairs.push((entry, *orig_entry)),
                None if quilt && entry.path().starts_with(".pc") => {}
                None => return,
            }
        }

        let touched = pairs
            .iter()
            .any(|(entry, orig_entry)| content(patched, entry) != content(orig, orig_entry));
        if !touched {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// The bytes of a regular file or hardlink; `None` for other kinds and for
/// bytes that cannot be read. Borrowed from the tree when it holds them in
/// memory, so comparing two trees copies nothing.
fn content<'t>(tree: &'t FileTree, entry: &'t Entry) -> Option<Cow<'t, [u8]>> {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
        .then(|| tree.bytes(entry).ok())
        .flatten()
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
