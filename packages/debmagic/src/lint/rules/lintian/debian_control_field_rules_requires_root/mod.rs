//! Rules for lintian's `debian/control/field/rules-requires-root` Check.

mod ln1258_spelling_error_in_rules_requires_root;
mod ln1667_redundant_rules_requires_root_no_field;

pub(crate) use ln1258_spelling_error_in_rules_requires_root::SpellingErrorInRulesRequiresRoot;
pub(crate) use ln1667_redundant_rules_requires_root_no_field::RedundantRulesRequiresRootNoField;
