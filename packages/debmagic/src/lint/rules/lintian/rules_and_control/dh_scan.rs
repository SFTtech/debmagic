//! What lintian's `debhelper` Check records while it walks `debian/rules`.
//!
//! One pass over [`debhelper_lines`] collects the facts several Tags report
//! after the walk: addons, `dh_clean -k`, `--parallel`, overrides, and a
//! `DH_COMPAT` assignment. The branch order is lintian's `elsif` chain, so a
//! line that is a debhelper command or a `dh` sequencer is not also an
//! override target.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::debhelper::DebhelperFacts;

use super::rules_text::{debhelper_lines, read_rules};

static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+[@+-]?(?:\S+=\S+\s+)*(dh_\S+)").expect("valid regex"));
static DH_SEQUENCER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("valid regex"));
static CLEAN_K: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\s+-k(?:\s+.*)?\n?$").expect("valid regex"));
static WITH_ADDON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s--with(?:=|\s+)(\S+)").expect("valid regex"));
static EXPORT_DH_COMPAT_ASSIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*export\s+DH_COMPAT\s*:?=\s*(\S+)").expect("valid regex"));
static EXPORT_DH_COMPAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*export\s+DH_COMPAT").expect("valid regex"));
static DH_COMPAT_ASSIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*DH_COMPAT\s*:?=\s*(\S+)").expect("valid regex"));
static SPACED_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^:]*(?:override|execute_(?:after|before))\s+dh_[^:]*:").expect("valid regex")
});
static DH_TARGET_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^:]*_dh_[^:]*):").expect("valid regex"));
static OVERRIDE_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:override|execute_(?:before|after))_dh_([^\s]+?)(-arch|-indep|)$")
        .expect("valid regex")
});

/// An `override_dh_*` style target: where it was last seen and its focus
/// (`-arch`, `-indep`, or empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OverrideTarget {
    pub(super) position: u32,
    pub(super) focus: String,
}

#[derive(Debug, Default)]
pub(super) struct DhScan {
    /// Positions of `dh_installmanpages` lines.
    pub(super) installmanpages: Vec<u32>,
    /// A `dh_clean` line passes `-k`.
    pub(super) clean_k: bool,
    /// `(position, addon as written)` for every `--with quilt`.
    pub(super) quilt_addons: Vec<(u32, String)>,
    /// Last position of `--with systemd`, 0 when absent.
    pub(super) systemd_position: u32,
    /// Last position of `--parallel` on a `dh` line, 0 when absent.
    pub(super) parallel_position: u32,
    /// `dh_<name>` to its last override target.
    pub(super) overrides: HashMap<String, OverrideTarget>,
    /// Value lintian takes from a `DH_COMPAT` make assignment.
    pub(super) dh_compat_rules: String,
    /// `dh_autotools-dev_*` command or the `autotools_dev` addon was used.
    pub(super) uses_autotools_dev_dh: bool,
    /// Perl's `$uses_debhelper`: a dh command, the sequencer, or an override.
    pub(super) uses_debhelper: bool,
    /// A `dh` sequencer line was seen (Perl's `$seen_dh_sequencer`).
    pub(super) sequencer: bool,
}

/// Debhelper facts and the `debian/rules` walk, for a tree where lintian
/// reaches the end of the walk: `debian/rules` is readable, `Build-Driver` is
/// `debian-rules` or absent, and a dh command was seen.
pub(super) fn gated_scan(ctx: &mut SourceContext<'_>) -> Option<(DebhelperFacts, DhScan)> {
    let facts = ctx.debhelper()?;
    if !facts.uses_debhelper() {
        return None;
    }
    let contents = ctx.files().and_then(read_rules)?;
    Some((facts, scan(&contents)))
}

/// Walk `debian/rules` as lintian's `debhelper` Check does.
pub(super) fn scan(contents: &str) -> DhScan {
    let mut scan = DhScan::default();
    let mut dh_compat_variable = String::new();
    for (position, line) in debhelper_lines(contents) {
        if let Some(found) = DH_COMMAND.captures(&line)
            && !found[1].starts_with("dh_autoreconf")
        {
            let command = &found[1];
            if command == "dh_installmanpages" {
                scan.installmanpages.push(position);
            }
            if command == "dh_autotools-dev_restoreconfig"
                || command == "dh_autotools-dev_updateconfig"
            {
                scan.uses_autotools_dev_dh = true;
            }
            if command == "dh_clean" && CLEAN_K.is_match(&line) {
                scan.clean_k = true;
            }
            scan.uses_debhelper = true;
        } else if DH_SEQUENCER.is_match(&line) {
            scan.uses_debhelper = true;
            scan.sequencer = true;
            if line.contains("--parallel") {
                scan.parallel_position = position;
            }
            for found in WITH_ADDON.captures_iter(&line) {
                for written in addon_list(&found[1]).split(',') {
                    let addon = written.replace('-', "_");
                    if addon == "autotools_dev" {
                        scan.uses_autotools_dev_dh = true;
                    }
                    if addon == "systemd" {
                        scan.systemd_position = position;
                    }
                    if addon == "quilt" {
                        scan.quilt_addons.push((position, written.to_string()));
                    }
                }
            }
        } else if let Some(found) = EXPORT_DH_COMPAT_ASSIGN.captures(&line) {
            scan.dh_compat_rules = found[1].to_string();
        } else if EXPORT_DH_COMPAT.is_match(&line) {
            if truthy(&dh_compat_variable) {
                scan.dh_compat_rules = dh_compat_variable.clone();
            }
        } else if let Some(found) = DH_COMPAT_ASSIGN.captures(&line) {
            dh_compat_variable = found[1].to_string();
            if truthy(&scan.dh_compat_rules) {
                scan.dh_compat_rules = found[1].to_string();
            }
        } else if SPACED_TARGET.is_match(&line) {
            // A typo, reported by another Rule.
        } else if let Some(found) = DH_TARGET_LINE.captures(&line) {
            for target in found[1]
                .split_whitespace()
                .filter(|target| target.contains("_dh_") && !target.contains('%'))
            {
                let Some(parts) = OVERRIDE_TARGET.captures(target) else {
                    continue;
                };
                scan.overrides.insert(
                    format!("dh_{}", &parts[1]),
                    OverrideTarget {
                        position,
                        focus: parts[2].to_string(),
                    },
                );
                scan.uses_debhelper = true;
            }
        }
    }
    scan
}

/// Perl truthiness of a string: empty and `0` are false.
fn truthy(value: &str) -> bool {
    !value.is_empty() && value != "0"
}

/// The addon list of a `--with` match: `(['"]?)(\S+)\1` backtracks to the last
/// closing quote that matches the opening one.
fn addon_list(token: &str) -> &str {
    for quote in ['\'', '"'] {
        if let Some(rest) = token.strip_prefix(quote)
            && let Some(end) = rest.rfind(quote)
            && end > 0
        {
            return &rest[..end];
        }
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_addon_lists_lose_their_quotes() {
        assert_eq!(addon_list("'quilt,python3'"), "quilt,python3");
        assert_eq!(addon_list("\"quilt\""), "quilt");
        assert_eq!(addon_list("quilt"), "quilt");
        assert_eq!(addon_list("'quilt"), "'quilt");
    }

    #[test]
    fn overrides_keep_the_last_position_and_focus() {
        let scan = scan("override_dh_foo:\noverride_dh_foo-arch override_dh_bar:\n");
        assert_eq!(scan.overrides.len(), 2);
        assert_eq!(
            scan.overrides["dh_foo"],
            OverrideTarget {
                position: 2,
                focus: "-arch".to_string()
            }
        );
        assert!(scan.uses_debhelper);
    }

    #[test]
    fn dh_compat_variable_is_exported_later() {
        let scan = scan("DH_COMPAT := 7\nexport DH_COMPAT\n");
        assert_eq!(scan.dh_compat_rules, "7");
    }
}
