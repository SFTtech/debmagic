//! Rules for lintian's `fields/vcs` Check.
//!
//! Each Rule owns one Tag. The Source package's Dsc is read when there is one,
//! otherwise the source paragraph of `debian/control`.

mod ln0797_vcs_field_bitrotted;
mod ln0798_vcs_field_mismatch;
mod ln0799_orphaned_package_maintained_in_private_space;
mod ln0800_orphaned_package_not_maintained_in_debian_infrastructure;
mod ln1240_vcs_browser_links_to_empty_view;
mod ln1241_vcs_field_has_unexpected_spaces;
mod ln1242_vcs_field_uses_unknown_uri_format;
mod ln1243_vcs_git_uses_invalid_user_uri;
mod ln1371_vcs_field_not_canonical;
mod ln1372_vcs_field_uses_insecure_uri;
mod ln1373_vcs_fields_use_more_than_one_vcs;
mod ln1660_co_maintained_package_with_no_vcs_fields;

pub(crate) use ln0797_vcs_field_bitrotted::VcsFieldBitrotted;
pub(crate) use ln0798_vcs_field_mismatch::VcsFieldMismatch;
pub(crate) use ln0799_orphaned_package_maintained_in_private_space::OrphanedPackageMaintainedInPrivateSpace;
pub(crate) use ln0800_orphaned_package_not_maintained_in_debian_infrastructure::OrphanedPackageNotMaintainedInDebianInfrastructure;
pub(crate) use ln1240_vcs_browser_links_to_empty_view::VcsBrowserLinksToEmptyView;
pub(crate) use ln1241_vcs_field_has_unexpected_spaces::VcsFieldHasUnexpectedSpaces;
pub(crate) use ln1242_vcs_field_uses_unknown_uri_format::VcsFieldUsesUnknownUriFormat;
pub(crate) use ln1243_vcs_git_uses_invalid_user_uri::VcsGitUsesInvalidUserUri;
pub(crate) use ln1371_vcs_field_not_canonical::VcsFieldNotCanonical;
pub(crate) use ln1372_vcs_field_uses_insecure_uri::VcsFieldUsesInsecureUri;
pub(crate) use ln1373_vcs_fields_use_more_than_one_vcs::VcsFieldsUseMoreThanOneVcs;
pub(crate) use ln1660_co_maintained_package_with_no_vcs_fields::CoMaintainedPackageWithNoVcsFields;
