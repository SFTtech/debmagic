use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::Config;

use super::rule::Code;
use super::rule::Severity;
use super::selection::resolve_selected_codes;
use super::subject::{LintTarget, resolve_target};

/// Whether the Lintian override files of each processable suppress Diagnostics.
///
/// `Hide` is the default: an overridden Diagnostic is kept but not printed.
/// `Show` prints it flagged as overridden. `Off` skips the override pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverrideMode {
    #[default]
    Hide,
    Show,
    Off,
}

/// Clap-free inputs for resolving a [`LintIntent`].
#[derive(Debug, Clone)]
pub struct LintIntentInput {
    /// Directory used when `subjects` is empty (typically cwd).
    pub fallback_dir: PathBuf,
    /// Paths to lint: one Source tree, or any number of artifacts (`.deb`,
    /// `.udeb`, `.ddeb`, `.dsc`, `.changes`, `.buildinfo`).
    pub subjects: Vec<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub select: Vec<String>,
    pub ignore: Vec<String>,
    pub fail_on: Vec<Severity>,
    /// Worker threads for running Rules; `None` uses the available parallelism.
    pub threads: Option<usize>,
    /// Report bringup, initialization shared across Rules, and per-Rule time.
    pub timings: bool,
    /// How Lintian override files are applied; see [`OverrideMode`].
    pub overrides: OverrideMode,
}

/// Fully resolved description of *how* a lint runs.
#[derive(Debug, Clone)]
pub struct LintIntent {
    pub target: LintTarget,
    pub selected: Vec<Code>,
    pub fail_on: HashSet<Severity>,
    pub severity_remaps: HashMap<Code, Severity>,
    /// Worker threads for running Rules; `None` uses the available parallelism.
    pub threads: Option<usize>,
    /// Report bringup, initialization shared across Rules, and per-Rule time.
    pub timings: bool,
    /// How Lintian override files are applied; see [`OverrideMode`].
    pub overrides: OverrideMode,
}

fn resolve_fail_on(fail_on: &[Severity]) -> HashSet<Severity> {
    if fail_on.is_empty() {
        HashSet::from([Severity::Error])
    } else {
        fail_on.iter().copied().collect()
    }
}

pub fn resolve_lint_intent(input: LintIntentInput) -> anyhow::Result<LintIntent> {
    let subjects = if input.subjects.is_empty() {
        vec![input.fallback_dir]
    } else {
        input.subjects
    };
    let target = resolve_target(subjects)?;

    let config_source_dir = match &target {
        LintTarget::SourceTree(path) => Some(path.as_path()),
        LintTarget::Artifacts(_) => None,
    };
    let _config = Config::load(config_source_dir, input.config_file.as_deref())?;

    Ok(LintIntent {
        target,
        selected: resolve_selected_codes(&input.select, &input.ignore)?,
        fail_on: resolve_fail_on(&input.fail_on),
        severity_remaps: HashMap::new(),
        threads: input.threads,
        timings: input.timings,
        overrides: input.overrides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::rule::RuleMeta;
    use crate::lint::rules::DebmagicDummyTrigger;
    use crate::lint::subject::SubjectKind;

    fn asset_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("config1.toml")
    }

    fn base_input(fallback: PathBuf) -> LintIntentInput {
        LintIntentInput {
            fallback_dir: fallback,
            subjects: Vec::new(),
            config_file: Some(asset_config()),
            select: vec![],
            ignore: vec![],
            fail_on: vec![],
            threads: None,
            timings: false,
            overrides: OverrideMode::default(),
        }
    }

    #[test]
    fn resolve_absolutizes_source_tree_subject() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_lint_intent(base_input(dir.clone()))?;
        assert_eq!(intent.target.single_kind(), Some(SubjectKind::SourceTree));
        assert_eq!(
            intent.target,
            LintTarget::SourceTree(std::path::absolute(&dir)?)
        );
        Ok(())
    }

    #[test]
    fn resolve_keeps_every_artifact_path() -> anyhow::Result<()> {
        let mut input = base_input(std::env::temp_dir());
        input.subjects = vec![PathBuf::from("a_1.0_amd64.changes"), PathBuf::from("a.deb")];
        let intent = resolve_lint_intent(input)?;
        let LintTarget::Artifacts(subjects) = intent.target else {
            panic!("expected artifacts");
        };
        assert_eq!(subjects.len(), 2);
        Ok(())
    }

    #[test]
    fn resolve_default_fail_on_is_error_only() -> anyhow::Result<()> {
        let intent = resolve_lint_intent(base_input(std::env::temp_dir()))?;
        assert_eq!(intent.fail_on, HashSet::from([Severity::Error]));
        Ok(())
    }

    #[test]
    fn resolve_carries_override_mode() -> anyhow::Result<()> {
        let mut input = base_input(std::env::temp_dir());
        assert_eq!(
            resolve_lint_intent(input.clone())?.overrides,
            OverrideMode::Hide
        );
        input.overrides = OverrideMode::Show;
        assert_eq!(
            resolve_lint_intent(input.clone())?.overrides,
            OverrideMode::Show
        );
        input.overrides = OverrideMode::Off;
        assert_eq!(resolve_lint_intent(input)?.overrides, OverrideMode::Off);
        Ok(())
    }

    #[test]
    fn resolve_selected_contains_dm0001_by_default() -> anyhow::Result<()> {
        let intent = resolve_lint_intent(base_input(std::env::temp_dir()))?;
        assert!(intent.selected.contains(&DebmagicDummyTrigger::CODE));
        Ok(())
    }
}
