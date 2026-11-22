//! AppStream metadata files of a Binary package: the XML files in
//! `usr/share/metainfo/` and the legacy `usr/share/appdata/`.
//!
//! This is the file selection of lintian 2.144.0's `appstream-metadata` Check.
//! The metainfo directory contributes its direct children, the legacy
//! directory every file below it. Only regular files and hardlinks count
//! (lintian's `is_file`); symlinks are skipped. Each file is an
//! [`XmlDocument`] whose parse is lazy, so the Rules that share this value
//! parse each file once.

// The Rules that read these accessors land with the appstream-metadata Tags.
#![allow(dead_code)]

use std::path::Path;

use super::file_tree::FileTree;
use super::xml::XmlDocument;

const METAINFO_DIR: &str = "usr/share/metainfo";
const LEGACY_DIR: &str = "usr/share/appdata";

/// The AppStream metadata files of one Binary package's Data files.
#[derive(Default)]
pub struct AppstreamFiles {
    metainfo: Vec<XmlDocument>,
    legacy: Vec<XmlDocument>,
}

impl AppstreamFiles {
    /// The files of `tree`: direct children of `usr/share/metainfo`, and
    /// every file below `usr/share/appdata`.
    pub fn from_tree(tree: &FileTree) -> Self {
        let metainfo = documents(tree, |path| path.parent() == Some(Path::new(METAINFO_DIR)));
        let legacy = documents(tree, |path| path.starts_with(LEGACY_DIR));
        Self { metainfo, legacy }
    }

    /// Files in `usr/share/metainfo`, in path order.
    pub fn metainfo(&self) -> &[XmlDocument] {
        &self.metainfo
    }

    /// Files in `usr/share/appdata`, in path order.
    pub fn legacy(&self) -> &[XmlDocument] {
        &self.legacy
    }

    /// Every file, metainfo first, as lintian visits them.
    pub fn all(&self) -> impl Iterator<Item = &XmlDocument> {
        self.metainfo.iter().chain(self.legacy.iter())
    }
}

fn documents(tree: &FileTree, selects: impl Fn(&Path) -> bool) -> Vec<XmlDocument> {
    tree.entries()
        .iter()
        .filter(|entry| entry.is_file() && selects(entry.path()))
        .filter_map(|entry| XmlDocument::read(tree, entry))
        .collect()
}
