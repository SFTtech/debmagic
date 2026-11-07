//! Rules for the `Description` field, matching lintian's `fields/description` Check.
//!
//! `description_field` splits the field the way that Check does. Each Rule
//! owns the predicate for its Tag.

mod description_field;
mod ln0037_extended_description_is_empty;
mod ln0050_description_synopsis_is_empty;
mod ln0051_description_starts_with_package_name;
mod ln0052_description_synopsis_starts_with_article;
mod ln0053_description_too_short;
mod ln0054_description_is_pkg_name;
mod ln0055_synopsis_too_long;
mod ln0056_extended_description_line_too_long;
mod ln0432_extended_description_contains_empty_paragraph;

pub(crate) use ln0037_extended_description_is_empty::ExtendedDescriptionIsEmpty;
pub(crate) use ln0050_description_synopsis_is_empty::DescriptionSynopsisIsEmpty;
pub(crate) use ln0051_description_starts_with_package_name::DescriptionStartsWithPackageName;
pub(crate) use ln0052_description_synopsis_starts_with_article::DescriptionSynopsisStartsWithArticle;
pub(crate) use ln0053_description_too_short::DescriptionTooShort;
pub(crate) use ln0054_description_is_pkg_name::DescriptionIsPkgName;
pub(crate) use ln0055_synopsis_too_long::SynopsisTooLong;
pub(crate) use ln0056_extended_description_line_too_long::ExtendedDescriptionLineTooLong;
pub(crate) use ln0432_extended_description_contains_empty_paragraph::ExtendedDescriptionContainsEmptyParagraph;
