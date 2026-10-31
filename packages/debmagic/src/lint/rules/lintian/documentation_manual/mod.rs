//! Manual-page path Rules from lintian's `documentation/manual` Check.
//!
//! These Tags judge where a page lives, how it is named and compressed, and
//! whether a command in the package has a page. Groff, spelling, whatis, and
//! executable-bit Tags from the same Check are separate. Udebs are skipped:
//! the Check returns before it walks installed files.

mod ln0152_no_manual_page;
mod ln0153_uncompressed_manual_page;
mod ln0154_wrong_compression_in_manual_page;
mod ln0155_country_in_manual;
mod ln0156_build_path_in_manual;
mod ln0157_broken_link_to_undocumented;
mod manual;

pub(crate) use ln0152_no_manual_page::NoManualPage;
pub(crate) use ln0153_uncompressed_manual_page::UncompressedManualPage;
pub(crate) use ln0154_wrong_compression_in_manual_page::WrongCompressionInManualPage;
pub(crate) use ln0155_country_in_manual::CountryInManual;
pub(crate) use ln0156_build_path_in_manual::BuildPathInManual;
pub(crate) use ln0157_broken_link_to_undocumented::BrokenLinkToUndocumented;
