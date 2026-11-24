//! Rules for symlink targets, matching lintian's `files/symbolic-links` Check.
//! Most judge Binary package Data files; `absolute-symbolic-link-target-in-source`
//! judges the packaging File tree.
//!
//! `links` judges the link text once per symlink. `package-contains-unsafe-symlink`
//! is the nested `files/symbolic-links/broken` Check: it resolves the target
//! instead of judging the text.

mod links;
mod ln0006_package_contains_unsafe_symlink;
mod ln0029_symlink_contains_spurious_segments;
mod ln0030_symlink_is_self_recursive;
mod ln0031_lengthy_symlink;
mod ln0064_absolute_symlink_in_top_level_folder;
mod ln0068_relative_symlink;
mod ln0069_symlink_ends_with_slash;
mod ln0070_symlink_has_double_slash;
mod ln0071_symlink_has_too_many_up_segments;
mod ln0075_compressed_symlink_with_wrong_ext;
mod ln0733_absolute_symbolic_link_target_in_source;
mod ln0842_wayward_symbolic_link_target_in_source;
mod ln1265_symlink_target_in_build_tree;
mod ln1266_symlink_target_in_tmp;
mod ln1504_package_contains_broken_symlink_wildcard;

pub(crate) use ln0006_package_contains_unsafe_symlink::PackageContainsUnsafeSymlink;
pub(crate) use ln0029_symlink_contains_spurious_segments::SymlinkContainsSpuriousSegments;
pub(crate) use ln0030_symlink_is_self_recursive::SymlinkIsSelfRecursive;
pub(crate) use ln0031_lengthy_symlink::LengthySymlink;
pub(crate) use ln0064_absolute_symlink_in_top_level_folder::AbsoluteSymlinkInTopLevelFolder;
pub(crate) use ln0068_relative_symlink::RelativeSymlink;
pub(crate) use ln0069_symlink_ends_with_slash::SymlinkEndsWithSlash;
pub(crate) use ln0070_symlink_has_double_slash::SymlinkHasDoubleSlash;
pub(crate) use ln0071_symlink_has_too_many_up_segments::SymlinkHasTooManyUpSegments;
pub(crate) use ln0075_compressed_symlink_with_wrong_ext::CompressedSymlinkWithWrongExt;
pub(crate) use ln0733_absolute_symbolic_link_target_in_source::AbsoluteSymbolicLinkTargetInSource;
pub(crate) use ln0842_wayward_symbolic_link_target_in_source::WaywardSymbolicLinkTargetInSource;
pub(crate) use ln1265_symlink_target_in_build_tree::SymlinkTargetInBuildTree;
pub(crate) use ln1266_symlink_target_in_tmp::SymlinkTargetInTmp;
pub(crate) use ln1504_package_contains_broken_symlink_wildcard::PackageContainsBrokenSymlinkWildcard;
