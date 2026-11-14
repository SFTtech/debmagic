//! Rules for lintian's `debhelper` Check that read `debian/rules` targets and
//! packaging-tree maintainer scripts.
//!
//! Compat level and whether `debian/rules` invokes dh come from
//! [`crate::lint::debhelper::DebhelperFacts`]. Each Rule owns one Tag.

use crate::lint::debian_control::DebianControl;

mod commands;
mod ln0176_debhelper_base_compat_level_is_missing;
mod ln0177_typo_in_debhelper_override_target;
mod ln0178_maintainer_script_lacks_debhelper_token;

pub(crate) use ln0176_debhelper_base_compat_level_is_missing::DebhelperBaseCompatLevelIsMissing;
pub(crate) use ln0177_typo_in_debhelper_override_target::TypoInDebhelperOverrideTarget;
pub(crate) use ln0178_maintainer_script_lacks_debhelper_token::MaintainerScriptLacksDebhelperToken;

/// Lintian scans `debian/rules` only when `Build-Driver` is absent or
/// `debian-rules`. Any other declared driver returns before the rules loop.
pub(super) fn scans_debian_rules(control: Option<&DebianControl>) -> bool {
    let Some(driver) = control.and_then(|control| control.source().get("Build-Driver")) else {
        return true;
    };
    driver.trim() == "debian-rules"
}
