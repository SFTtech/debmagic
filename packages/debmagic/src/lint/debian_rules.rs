use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Policy 4.9 required `debian/rules` targets. Extra on
/// `debian-rules-missing-required-target` is one of these names.
pub const POLICY_TARGETS: &[&str] = &[
    "build",
    "build-arch",
    "build-indep",
    "binary",
    "binary-arch",
    "binary-indep",
    "clean",
];

/// Parsed `debian/rules` for Policy target presence.
///
/// Missing files, unreadable files, and non-UTF-8 contents are unavailable
/// (lintian returns before scanning). An empty file is available and reports
/// no missing targets. Unknown makefile includes suppress missing-target
/// reporting (lintian will not chase those files).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebianRules {
    missing_policy_targets: Vec<String>,
}

impl DebianRules {
    pub fn load(path: &Path) -> Option<Self> {
        if !path.is_file() {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?;
        Some(Self::parse(text))
    }

    pub fn parse(contents: &str) -> Self {
        Self {
            missing_policy_targets: missing_policy_targets(contents),
        }
    }

    pub fn missing_policy_targets(&self) -> &[String] {
        &self.missing_policy_targets
    }
}

/// Paths from lintian 2.139.0 `data/rules/known-makefiles`. Values are
/// makefile-provided target names; only Policy targets among them count as
/// present.
fn known_makefile_targets(makefile: &str) -> Option<&'static [&'static str]> {
    for &(path, targets) in KNOWN_MAKEFILES {
        if path == makefile {
            return Some(targets);
        }
    }
    None
}

const KNOWN_MAKEFILES: &[(&str, &[&str])] = &[
    ("/usr/share/cli-common/cli-nant.make", &[]),
    ("/usr/share/cli-common/cli.make", &[]),
    ("/usr/share/coq/coqvars.mk", &[]),
    ("/usr/share/dpkg/architecture.mk", &[]),
    ("/usr/share/dpkg/buildflags.mk", &[]),
    ("/usr/share/dpkg/default.mk", &[]),
    ("/usr/share/dpkg/pkg-info.mk", &[]),
    ("/usr/share/dpkg/vendor.mk", &[]),
    ("/usr/share/gcj/debian_defaults", &[]),
    ("/usr/share/hardening-includes/hardening.make", &[]),
    ("/usr/share/javahelper/java-vars.mk", &[]),
    ("/usr/share/libdbi-perl/perl-dbdabi.make", &[]),
    ("/usr/share/mpi-default-dev/debian_defaults", &[]),
    ("/usr/share/ocaml/ocamlinit.mk", &[]),
    ("/usr/share/ocaml/ocamlvars.mk", &[]),
    ("/usr/share/octave/debian/defs.make", &[]),
    ("/usr/share/pkg-kde-tools/makefiles/1/variables.mk", &[]),
    ("/usr/share/postgresql-common/pgxs_debian_control.mk", &[]),
    ("/usr/share/python3/python.mk", &[]),
    (
        "/usr/share/quilt/quilt.make",
        &["patch", "unpatch", "$(QUILT_STAMPFN)"],
    ),
];

fn missing_policy_targets(contents: &str) -> Vec<String> {
    if contents.is_empty() {
        return Vec::new();
    }

    let mut seen: HashSet<&'static str> = HashSet::new();
    let mut unknown_includes = false;
    let mut variables: HashMap<String, String> = HashMap::new();

    for line in complete_lines(contents) {
        if let Some(makefile) = include_makefile(&line) {
            if let Some(targets) = known_makefile_targets(makefile) {
                for target in targets {
                    mark_policy(&mut seen, target);
                }
            } else {
                unknown_includes = true;
            }
        }

        if let Some((name, value)) = assignment(&line) {
            variables.insert(name, value);
        }

        if let Some(targets) = phony_targets(&line) {
            // `.PHONY: name` is enough for GNU make. Lintian's variable
            // expansion on this line uses the `$(VAR)` form as the seen
            // key, so it never marks Policy targets from `.PHONY: $(TARGETS)`.
            mark_declared_targets(&mut seen, &targets, &variables, false);
            continue;
        }

        if CONDITIONAL.is_match(&line) {
            continue;
        }

        if let Some(targets) = target_names(&line) {
            mark_declared_targets(&mut seen, &targets, &variables, true);
        }
    }

    if unknown_includes {
        return Vec::new();
    }

    POLICY_TARGETS
        .iter()
        .filter(|target| !seen.contains(*target))
        .map(|target| (*target).to_string())
        .collect()
}

fn complete_lines(contents: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut continued = String::new();
    for line in contents.split('\n') {
        if COMMENT.is_match(line) {
            continue;
        }
        let line = if continued.is_empty() {
            line.to_string()
        } else {
            let mut joined = std::mem::take(&mut continued);
            joined.push_str(line);
            joined
        };
        if let Some(stripped) = line.strip_suffix('\\') {
            continued = stripped.to_string();
            continue;
        }
        lines.push(line);
    }
    lines
}

fn include_makefile(line: &str) -> Option<&str> {
    INCLUDE
        .captures(line)
        .and_then(|captures| captures.get(1))
        .map(|makefile| makefile.as_str())
}

fn assignment(line: &str) -> Option<(String, String)> {
    let captures = ASSIGNMENT.captures(line)?;
    Some((captures[1].to_string(), captures[2].to_string()))
}

fn phony_targets(line: &str) -> Option<Vec<String>> {
    let captures = PHONY.captures(line)?;
    Some(split_words(&captures[1]))
}

fn target_names(line: &str) -> Option<Vec<String>> {
    let captures = TARGET.captures(line)?;
    Some(split_words(&captures[1]))
}

fn split_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(ToString::to_string).collect()
}

fn mark_declared_targets(
    seen: &mut HashSet<&'static str>,
    targets: &[String],
    variables: &HashMap<String, String>,
    expand_variables: bool,
) {
    for target in targets {
        if target.contains('%') {
            mark_percent(seen, target);
            continue;
        }
        if target == ".DEFAULT" {
            for policy in POLICY_TARGETS {
                seen.insert(*policy);
            }
            continue;
        }
        if expand_variables
            && let Some(name) = variable_ref(target)
            && let Some(value) = variables.get(name)
        {
            for word in value.split_whitespace() {
                mark_policy(seen, word);
            }
            break;
        }
        mark_policy(seen, target);
    }
}

fn mark_percent(seen: &mut HashSet<&'static str>, target: &str) {
    let pattern = target
        .split('%')
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(".*");
    let Ok(regex) = Regex::new(&pattern) else {
        return;
    };
    for policy in POLICY_TARGETS {
        if regex.is_match(policy) {
            seen.insert(*policy);
        }
    }
}

fn mark_policy(seen: &mut HashSet<&'static str>, name: &str) {
    if let Some(policy) = POLICY_TARGETS
        .iter()
        .copied()
        .find(|policy| *policy == name)
    {
        seen.insert(policy);
    }
}

fn variable_ref(target: &str) -> Option<&str> {
    VAR_REF
        .captures(target)
        .and_then(|captures| captures.get(1))
        .map(|name| name.as_str())
}

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));
static INCLUDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[s-]?include\s+(\S+)").expect("regex"));
static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:\S+\s+)*?(\S+)\s*[:?+]?=\s*(.*)$").expect("regex"));
static PHONY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("regex"));
static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^\s:][^:]*):+(.*)").expect("regex"));
static VAR_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\$[\(\{]([^\)\}]+)[\}\)]$").expect("regex"));

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn missing(contents: &str) -> Vec<String> {
        DebianRules::parse(contents)
            .missing_policy_targets()
            .to_vec()
    }

    #[test]
    fn missing_file_is_unavailable() {
        let path = std::env::temp_dir().join(format!("debmagic-no-rules-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        assert!(DebianRules::load(&path).is_none());
    }

    #[test]
    fn empty_file_reports_no_missing_targets() {
        assert!(missing("").is_empty());
    }

    #[test]
    fn shebang_only_misses_every_policy_target() {
        assert_eq!(
            missing("#!/usr/bin/make -f\n"),
            POLICY_TARGETS
                .iter()
                .map(|target| (*target).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn generic_empty_misses_arch_split_targets() {
        assert_eq!(
            missing(
                "#!/usr/bin/make -f\n\
                 build:\n\
                 binary:\n\
                 \tinstall -d debian/generic-empty\n\
                 \n\
                 clean:\n\
                 \trm -rf debian/generic-empty\n"
            ),
            vec![
                "build-arch".to_string(),
                "build-indep".to_string(),
                "binary-arch".to_string(),
                "binary-indep".to_string(),
            ]
        );
    }

    #[test]
    fn listed_targets_missing_build_arch_indep() {
        assert_eq!(
            missing(
                "#!/usr/bin/make -f\n\
                 \n\
                 build clean binary binary-arch binary-indep:\n\
                 \tdh $@\n"
            ),
            vec!["build-arch".to_string(), "build-indep".to_string()]
        );
    }

    #[test]
    fn percent_target_provides_every_policy_target() {
        assert!(missing("#!/usr/bin/make -f\n\n%:\n\tdh $@\n").is_empty());
    }

    #[test]
    fn unknown_include_suppresses_missing_targets() {
        assert!(missing("#!/usr/bin/make -f\n\n\tinclude debian/rules.mk\n").is_empty());
    }

    #[test]
    fn known_include_without_policy_targets_still_reports() {
        assert_eq!(
            missing(
                "#!/usr/bin/make -f\n\
                 include /usr/share/javahelper/java-vars.mk\n\
                 \n\
                 clean build binary:\n\
                 \tdh $@\n"
            ),
            vec![
                "build-arch".to_string(),
                "build-indep".to_string(),
                "binary-arch".to_string(),
                "binary-indep".to_string(),
            ]
        );
    }

    #[test]
    fn phony_listing_is_sufficient() {
        assert!(
            missing(
                "#!/usr/bin/make -f\n\
             .PHONY: binary binary-arch binary-indep build build-arch build-indep clean\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn variable_target_expands() {
        assert!(
            missing(
                "#!/usr/bin/make -f\n\
             \n\
               TARGETS := build clean binary binary-arch binary-indep build-arch build-indep\n\
             \n\
             $(TARGETS):\n\
             \tdh $@\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn default_target_provides_every_policy_target() {
        assert!(missing("#!/usr/bin/make -f\n\n.DEFAULT:\n\tdh $@\n").is_empty());
    }

    #[test]
    fn continued_target_line_is_joined() {
        assert!(
            missing(
                "#!/usr/bin/make -f\n\
             build clean binary \\\n\
             binary-arch binary-indep build-arch build-indep:\n\
             \tdh $@\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn load_reads_a_regular_file() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("rules");
        fs::write(&path, "#!/usr/bin/make -f\n\n%:\n\tdh $@\n").expect("write rules");
        let rules = DebianRules::load(&path).expect("load");
        assert!(rules.missing_policy_targets().is_empty());
    }
}
