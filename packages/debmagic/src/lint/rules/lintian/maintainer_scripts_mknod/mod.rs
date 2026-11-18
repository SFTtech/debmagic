//! Rules for lintian's `maintainer-scripts/mknod` Check.
//!
//! Each Rule owns one Tag and reads the Binary package's Control files.

mod ln0810_mknod_in_maintainer_script;

pub(crate) use ln0810_mknod_in_maintainer_script::MknodInMaintainerScript;
