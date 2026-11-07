//! Rules for lintian's `debian/po-debconf` Check.
//!
//! Each Rule reads the direct children of `debian/` in the Source tree or
//! Source package Patched files. Each Rule emits one Tag.

mod ln0375_untranslatable_debconf_templates;

pub(crate) use ln0375_untranslatable_debconf_templates::UntranslatableDebconfTemplates;
