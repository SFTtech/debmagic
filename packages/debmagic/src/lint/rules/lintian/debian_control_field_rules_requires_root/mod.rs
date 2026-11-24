//! Rules for lintian's `debian/control/field/rules-requires-root` Check.

mod ln1258_spelling_error_in_rules_requires_root;
mod ln1667_redundant_rules_requires_root_no_field;
mod ln1775_rules_silently_require_root;

pub(crate) use ln1258_spelling_error_in_rules_requires_root::SpellingErrorInRulesRequiresRoot;
pub(crate) use ln1667_redundant_rules_requires_root_no_field::RedundantRulesRequiresRootNoField;
pub(crate) use ln1775_rules_silently_require_root::RulesSilentlyRequireRoot;
