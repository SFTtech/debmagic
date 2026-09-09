use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::build_intent::load_config;

use super::rule::Code;
use super::rule::Severity;
use super::selection::resolve_selected_codes;
use super::subject::{Subject, resolve_subject};

/// Clap-free inputs for resolving a [`CheckIntent`].
#[derive(Debug, Clone)]
pub struct CheckIntentInput {
    /// Directory used when `subject` is unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub subject: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub select: Vec<String>,
    pub ignore: Vec<String>,
    pub fail_on: Vec<Severity>,
}

/// Fully resolved description of *how* a lint runs.
#[derive(Debug, Clone)]
pub struct CheckIntent {
    pub subject: Subject,
    pub selected: Vec<Code>,
    pub fail_on: HashSet<Severity>,
    pub severity_remaps: HashMap<Code, Severity>,
}

fn resolve_fail_on(fail_on: &[Severity]) -> HashSet<Severity> {
    if fail_on.is_empty() {
        HashSet::from([Severity::Error])
    } else {
        fail_on.iter().copied().collect()
    }
}

pub fn resolve_check_intent(input: CheckIntentInput) -> anyhow::Result<CheckIntent> {
    let subject_path = input.subject.unwrap_or(input.fallback_dir);
    let subject = resolve_subject(subject_path)?;

    let config_source_dir = match &subject {
        Subject::SourceTree(path) => Some(path.as_path()),
        Subject::BinaryPackage(_) | Subject::SourcePackage(_) => None,
    };
    let _config = load_config(config_source_dir, input.config_file.as_deref())?;

    Ok(CheckIntent {
        subject,
        selected: resolve_selected_codes(&input.select, &input.ignore)?,
        fail_on: resolve_fail_on(&input.fail_on),
        severity_remaps: HashMap::new(),
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

    fn base_input(fallback: PathBuf) -> CheckIntentInput {
        CheckIntentInput {
            fallback_dir: fallback,
            subject: None,
            config_file: Some(asset_config()),
            select: vec![],
            ignore: vec![],
            fail_on: vec![],
        }
    }

    #[test]
    fn resolve_absolutizes_source_tree_subject() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let intent = resolve_check_intent(base_input(dir.clone()))?;
        assert_eq!(intent.subject.kind(), SubjectKind::SourceTree);
        assert!(intent.subject.path().is_absolute());
        assert_eq!(intent.subject.path(), std::path::absolute(&dir)?);
        Ok(())
    }

    #[test]
    fn resolve_default_fail_on_is_error_only() -> anyhow::Result<()> {
        let intent = resolve_check_intent(base_input(std::env::temp_dir()))?;
        assert_eq!(intent.fail_on, HashSet::from([Severity::Error]));
        Ok(())
    }

    #[test]
    fn resolve_selected_contains_dm0001_by_default() -> anyhow::Result<()> {
        let intent = resolve_check_intent(base_input(std::env::temp_dir()))?;
        assert!(intent.selected.contains(&DebmagicDummyTrigger::CODE));
        Ok(())
    }
}
