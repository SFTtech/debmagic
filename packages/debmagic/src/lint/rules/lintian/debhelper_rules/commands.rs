//! Debhelper command names for override typos and maintainer-script rewrites.
//!
//! Names come from lintian's `data/debhelper/commands.json`. The `installed_by`
//! packages and the dh addon table are not loaded: a command is known when it
//! is listed, which is when `installed_by` is non-empty in that file.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

/// lintian `$LEVENSHTEIN_TOLERANCE`: a neighbour counts when the distance is
/// strictly below this.
const LEVENSHTEIN_TOLERANCE: usize = 3;

const TIMINGS: [&str; 3] = ["override", "execute_before", "execute_after"];
const FOCUSES: [&str; 3] = ["", "-arch", "-indep"];

static COMMANDS: LazyLock<Commands> = LazyLock::new(Commands::load);

static OVERRIDE_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(override|execute_(?:before|after))_dh_([^\s]+?)(-arch|-indep|)$")
        .expect("valid regex")
});

struct Commands {
    names: HashSet<String>,
    maint: HashSet<String>,
    known_targets: Vec<String>,
}

impl Commands {
    fn load() -> Self {
        let mut names = HashSet::new();
        let mut maint = HashSet::new();
        for line in include_str!("commands.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(name) = parts.next() else {
                continue;
            };
            names.insert(name.to_string());
            if parts.any(|part| part == "autoscript") {
                maint.insert(name.to_string());
            }
        }
        let mut known_targets = Vec::with_capacity(names.len() * TIMINGS.len() * FOCUSES.len());
        for name in &names {
            for focus in FOCUSES {
                for timing in TIMINGS {
                    known_targets.push(format!("{timing}_{name}{focus}"));
                }
            }
        }
        Self {
            names,
            maint,
            known_targets,
        }
    }
}

/// `dh_*` commands that rewrite maintainer scripts (`uses_autoscript`).
pub(super) fn is_maint_command(name: &str) -> bool {
    COMMANDS.maint.contains(name)
}

/// A makefile target lintian builds from every debhelper command: `override_`,
/// `execute_before_`, or `execute_after_`, plus an optional `-arch` / `-indep`.
pub(super) fn is_known_target(target: &str) -> bool {
    COMMANDS.known_targets.iter().any(|known| known == target)
}

/// Nearest known target within distance 2. Ties break toward the
/// lexicographically smaller name; the vendored recipes have a unique nearest.
pub(super) fn nearest_target(target: &str) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for known in &COMMANDS.known_targets {
        let distance = levenshtein(target, known);
        if distance >= LEVENSHTEIN_TOLERANCE {
            continue;
        }
        let replace = match best {
            None => true,
            Some((best_distance, best_name)) => {
                distance < best_distance
                    || (distance == best_distance && known.as_str() < best_name)
            }
        };
        if replace {
            best = Some((distance, known));
        }
    }
    best.map(|(_, name)| name.to_string())
}

/// `override_dh_cmd => override_dh_auto_cmd` when `dh_cmd` is not a command and
/// `dh_auto_cmd` is. `None` for a known command or when the `auto_` form is
/// absent.
pub(super) fn missing_auto(target: &str) -> Option<String> {
    let captures = OVERRIDE_TARGET.captures(target)?;
    let timing = &captures[1];
    let command = &captures[2];
    let dh_command = format!("dh_{command}");
    if COMMANDS.names.contains(&dh_command) {
        return None;
    }
    let auto = format!("dh_auto_{command}");
    if !COMMANDS.names.contains(&auto) {
        return None;
    }
    Some(format!("{timing}_{dh_command} => {timing}_{auto}"))
}

/// Classic Levenshtein distance (insert, delete, substitute). Lengths that
/// already differ by 3 or more return 3, which is outside the tolerance.
fn levenshtein(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let (left_len, right_len) = (left.len(), right.len());
    if left_len.abs_diff(right_len) >= LEVENSHTEIN_TOLERANCE {
        return LEVENSHTEIN_TOLERANCE;
    }
    let mut previous: Vec<usize> = (0..=right_len).collect();
    let mut current = vec![0; right_len + 1];
    for (i, left_char) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right.iter().enumerate() {
            let substitute = previous[j] + usize::from(left_char != right_char);
            current[j + 1] = substitute.min(current[j] + 1).min(previous[j + 1] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_typos_pick_the_vendored_nearest() {
        let cases = [
            ("oeverride_dh_missing", "override_dh_missing"),
            ("override_dh_testdirs", "override_dh_testdir"),
            ("override_dh_perls", "override_dh_perl"),
            ("override_dh_nump", "override_dh_numpy3"),
            (
                "override_dh_installxmlcatalog",
                "override_dh_installxmlcatalogs",
            ),
            ("override_dh_installdebs", "override_dh_installdeb"),
            ("override_dh_installcrons", "override_dh_installcron"),
            (
                "override_dh_install_examples",
                "override_dh_installexamples",
            ),
            (
                "override_dh_install_changelogs",
                "override_dh_installchangelogs",
            ),
            ("override_dh_instakk", "override_dh_install"),
            ("execute_before_dh_nope", "execute_before_dh_gnome"),
            ("execute_after_dh_nope", "execute_after_dh_gnome"),
        ];
        for (target, nearest) in cases {
            assert_eq!(nearest_target(target).as_deref(), Some(nearest), "{target}");
        }
        assert_eq!(nearest_target("override_dh_build"), None);
        assert_eq!(nearest_target("prefix_override_dh_gconfs"), None);
        assert_eq!(nearest_target("override_dh_will_never_exist"), None);
        assert!(is_known_target("override_dh_install"));
        assert!(!is_known_target("override_dh_instakk"));
    }

    #[test]
    fn missing_auto_suggests_dh_auto_build() {
        assert_eq!(
            missing_auto("override_dh_build").as_deref(),
            Some("override_dh_build => override_dh_auto_build")
        );
        assert_eq!(missing_auto("override_dh_install"), None);
        assert_eq!(missing_auto("override_dh_instakk"), None);
        assert_eq!(missing_auto("prefix_override_dh_gconfs"), None);
        assert!(is_maint_command("dh_installdeb"));
        assert!(!is_maint_command("dh_clean"));
        assert!(!is_maint_command("dh_autoreconf"));
    }
}
