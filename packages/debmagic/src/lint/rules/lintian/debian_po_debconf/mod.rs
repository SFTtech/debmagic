//! Rules for lintian's `debian/po-debconf` Check.
//!
//! Each Rule reads the direct children of `debian/` in the Source tree or
//! Source package Patched files. Each Rule emits one Tag.

mod ln0375_untranslatable_debconf_templates;
mod ln0407_stray_translated_debconf_templates;

pub(crate) use ln0375_untranslatable_debconf_templates::UntranslatableDebconfTemplates;
pub(crate) use ln0407_stray_translated_debconf_templates::StrayTranslatedDebconfTemplates;
