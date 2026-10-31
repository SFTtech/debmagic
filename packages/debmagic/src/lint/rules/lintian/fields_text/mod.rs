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

pub(crate) use ln0224_redundant_bugs_field::RedundantBugsField;
pub(crate) use ln0225_bad_homepage::BadHomepage;
pub(crate) use ln0226_description_contains_homepage::DescriptionContainsHomepage;
pub(crate) use ln0227_description_synopsis_is_duplicated::DescriptionSynopsisIsDuplicated;
pub(crate) use ln0228_field_too_long::FieldTooLong;
pub(crate) use ln0229_missing_separator_between_items::MissingSeparatorBetweenItems;
