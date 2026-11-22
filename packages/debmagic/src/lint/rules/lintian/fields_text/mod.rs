//! Field-text Rules: bugs, homepage, description lines, field length, and
//! raw relationship separators.
//!
//! `description_field` splits `Description` the same way as
//! `description/description_field`. Each Rule owns one Tag.

mod description_field;
mod ln0224_redundant_bugs_field;
mod ln0225_bad_homepage;
mod ln0226_description_contains_homepage;
mod ln0227_description_synopsis_is_duplicated;
mod ln0228_field_too_long;
mod ln0229_missing_separator_between_items;
mod ln0346_redundant_origin_field;
mod ln0347_essential_no_not_needed;
mod ln0348_new_essential_package;
mod ln0349_recommended_field;
mod ln0350_unknown_section;
mod ln0351_superfluous_clutter_in_homepage;
mod ln0431_excessive_priority_for_library_package;
mod ln0436_homepage_for_cpan_package_contains_version;
mod ln0437_homepage_refers_to_obsolete_debian_infrastructure;
mod ln0667_essential_in_source_package;
mod ln0668_explicit_default_in_package_type;
mod ln0924_section_is_dh_make_template;
mod ln1051_unknown_essential_value;
mod ln1061_priority_extra_is_replaced_by_priority_optional;
mod ln1062_unknown_priority;
mod ln1063_wrong_section_for_udeb;
mod ln1239_transitional_package_not_oldlibs_optional;
mod ln1365_homepage_field_uses_insecure_uri;

pub(crate) use ln0224_redundant_bugs_field::RedundantBugsField;
pub(crate) use ln0225_bad_homepage::BadHomepage;
pub(crate) use ln0226_description_contains_homepage::DescriptionContainsHomepage;
pub(crate) use ln0227_description_synopsis_is_duplicated::DescriptionSynopsisIsDuplicated;
pub(crate) use ln0228_field_too_long::FieldTooLong;
pub(crate) use ln0229_missing_separator_between_items::MissingSeparatorBetweenItems;
pub(crate) use ln0346_redundant_origin_field::RedundantOriginField;
pub(crate) use ln0347_essential_no_not_needed::EssentialNoNotNeeded;
pub(crate) use ln0348_new_essential_package::NewEssentialPackage;
pub(crate) use ln0349_recommended_field::RecommendedField;
pub(crate) use ln0350_unknown_section::UnknownSection;
pub(crate) use ln0351_superfluous_clutter_in_homepage::SuperfluousClutterInHomepage;
pub(crate) use ln0431_excessive_priority_for_library_package::ExcessivePriorityForLibraryPackage;
pub(crate) use ln0436_homepage_for_cpan_package_contains_version::HomepageForCpanPackageContainsVersion;
pub(crate) use ln0437_homepage_refers_to_obsolete_debian_infrastructure::HomepageRefersToObsoleteDebianInfrastructure;
pub(crate) use ln0667_essential_in_source_package::EssentialInSourcePackage;
pub(crate) use ln0668_explicit_default_in_package_type::ExplicitDefaultInPackageType;
pub(crate) use ln0924_section_is_dh_make_template::SectionIsDhMakeTemplate;
pub(crate) use ln1051_unknown_essential_value::UnknownEssentialValue;
pub(crate) use ln1061_priority_extra_is_replaced_by_priority_optional::PriorityExtraIsReplacedByPriorityOptional;
pub(crate) use ln1062_unknown_priority::UnknownPriority;
pub(crate) use ln1063_wrong_section_for_udeb::WrongSectionForUdeb;
pub(crate) use ln1239_transitional_package_not_oldlibs_optional::TransitionalPackageNotOldlibsOptional;
pub(crate) use ln1365_homepage_field_uses_insecure_uri::HomepageFieldUsesInsecureUri;
