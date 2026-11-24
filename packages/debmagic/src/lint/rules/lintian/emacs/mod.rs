//! Rules for lintian's `emacs` Check. Each Rule owns one Tag.

mod ln0837_bad_permissions_for_etc_emacs_script;

pub(crate) use ln0837_bad_permissions_for_etc_emacs_script::BadPermissionsForEtcEmacsScript;
