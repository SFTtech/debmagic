//! Rules for lintian's `debian/control/field/description/duplicate` Check.

mod ln1519_duplicate_long_description;
mod ln1520_duplicate_short_description;

pub(crate) use ln1519_duplicate_long_description::DuplicateLongDescription;
pub(crate) use ln1520_duplicate_short_description::DuplicateShortDescription;
