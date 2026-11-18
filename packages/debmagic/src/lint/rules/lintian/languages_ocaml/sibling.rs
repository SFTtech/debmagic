use std::path::{Path, PathBuf};

/// `<stem>.<extension>` in the directory of `path`, where `<stem>` is the file
/// name without its last extension.
pub(super) fn sibling(path: &Path, extension: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    Some(path.with_file_name(format!("{stem}.{extension}")))
}
