//! Manual-page Rules from lintian's `documentation/manual` Check.
//!
//! Path Tags judge where a page lives, how it is named and compressed, and
//! whether a command has a page. Content Tags read the page for groff,
//! whatis, `.so` links, and pod2man errors. Spelling and executable-bit Tags
//! from the same Check are separate. Udebs are skipped: the Check returns
//! before it walks installed files.

mod content;
mod ln0152_no_manual_page;
mod ln0153_uncompressed_manual_page;
mod ln0154_wrong_compression_in_manual_page;
mod ln0155_country_in_manual;
mod ln0156_build_path_in_manual;
mod ln0157_broken_link_to_undocumented;
mod ln0270_groff_message;
mod ln0271_empty_manual_page;
mod ln0272_undocumented_manual_page;
mod ln0273_bad_so_link_within_manual_page;
mod ln0274_bad_whatis_entry;
mod ln0275_manual_page_from_template;
mod ln0276_manual_page_with_generic_name;
mod ln0277_no_english_manual_page;
mod ln0278_pod_conversion_message;
mod ln0279_useless_whatis_entry;
mod ln0405_poor_compression_in_manual_page;
mod manual;

pub(crate) use ln0152_no_manual_page::NoManualPage;
pub(crate) use ln0153_uncompressed_manual_page::UncompressedManualPage;
pub(crate) use ln0154_wrong_compression_in_manual_page::WrongCompressionInManualPage;
pub(crate) use ln0155_country_in_manual::CountryInManual;
pub(crate) use ln0156_build_path_in_manual::BuildPathInManual;
pub(crate) use ln0157_broken_link_to_undocumented::BrokenLinkToUndocumented;
pub(crate) use ln0270_groff_message::GroffMessage;
pub(crate) use ln0271_empty_manual_page::EmptyManualPage;
pub(crate) use ln0272_undocumented_manual_page::UndocumentedManualPage;
pub(crate) use ln0273_bad_so_link_within_manual_page::BadSoLinkWithinManualPage;
pub(crate) use ln0274_bad_whatis_entry::BadWhatisEntry;
pub(crate) use ln0275_manual_page_from_template::ManualPageFromTemplate;
pub(crate) use ln0276_manual_page_with_generic_name::ManualPageWithGenericName;
pub(crate) use ln0277_no_english_manual_page::NoEnglishManualPage;
pub(crate) use ln0278_pod_conversion_message::PodConversionMessage;
pub(crate) use ln0279_useless_whatis_entry::UselessWhatisEntry;
pub(crate) use ln0405_poor_compression_in_manual_page::PoorCompressionInManualPage;
