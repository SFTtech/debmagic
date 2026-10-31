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
