//! Raw text of the copyright files that the `debian/copyright/dep5` Check reads.
//! Shared by the Rules whose Tags need the text rather than the parsed paragraphs.

use std::path::PathBuf;

use crate::lint::context::SourceContext;
use crate::lint::file_tree::EntryKind;

/// `debian/copyright` and `debian/<installable>.copyright` with their text.
/// Symlinks, non-files and files that are not UTF-8 are left out, as lintian
/// does (`is_symlink`, `is_valid_utf8`). Candidates match `debian_copyrights`.
pub(super) fn copyright_texts(ctx: &mut SourceContext<'_>) -> Vec<(PathBuf, String)> {
    let mut candidates = vec![PathBuf::from("debian/copyright")];
    if let Some(control) = ctx.debian_control() {
        candidates.extend(
            control.installables().iter().map(|installable| {
                PathBuf::from(format!("debian/{}.copyright", installable.name()))
            }),
        );
    }
    let Some(files) = ctx.files() else {
        return Vec::new();
    };
    candidates
        .into_iter()
        .filter_map(|path| {
            let entry = files.get(&path)?;
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?.to_string();
            Some((path, text))
        })
        .collect()
}
