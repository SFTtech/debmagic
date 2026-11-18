//! Rules for lintian's `debian/upstream/metadata` Check.
//!
//! Each Rule owns one Tag. Paths here are the packaging File tree.

mod ln0845_debian_upstream_obsolete_path;

pub(crate) use ln0845_debian_upstream_obsolete_path::DebianUpstreamObsoletePath;
