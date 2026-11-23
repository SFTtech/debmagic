//! Rules for lintian's `debian/lintian-overrides/comments` Check.

mod ln1761_spelling_in_override_comment;
mod ln1762_capitalization_in_override_comment;

pub(crate) use ln1761_spelling_in_override_comment::SpellingInOverrideComment;
pub(crate) use ln1762_capitalization_in_override_comment::CapitalizationInOverrideComment;
