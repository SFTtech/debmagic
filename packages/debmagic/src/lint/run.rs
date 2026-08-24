use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;

use anyhow::bail;

use super::context::LintContext;
use super::diagnostic::Diagnostic;
use super::intent::CheckIntent;
use super::registry;
use super::rule::Severity;
use super::subject::{Subject, SubjectKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Success,
    PolicyFailure,
}

pub fn run_check(intent: &CheckIntent) -> anyhow::Result<(Vec<Diagnostic>, CheckOutcome)> {
    match &intent.subject {
        Subject::BinaryPackage(path) => {
            bail!(
                "{} checks are not implemented (path: {})",
                SubjectKind::BinaryPackage.label(),
                path.display()
            );
        }
        Subject::SourcePackage(path) => {
            bail!(
                "{} checks are not implemented (path: {})",
                SubjectKind::SourcePackage.label(),
                path.display()
            );
        }
        Subject::SourceTree(path) => run_source_tree_check(intent, path),
    }
}

fn run_source_tree_check(
    intent: &CheckIntent,
    source_tree: &Path,
) -> anyhow::Result<(Vec<Diagnostic>, CheckOutcome)> {
    let debian_dir = source_tree.join("debian");
    if !debian_dir.is_dir() {
        bail!(
            "source directory {} requires a debian/ directory",
            source_tree.display()
        );
    }

    let selected = intent.selected.iter().copied().collect::<HashSet<_>>();
    let execution: Vec<_> = registry::all_rules()
        .iter()
        .copied()
        .filter(|rule| {
            selected.contains(&rule.code())
                && registry::rule_applies(*rule, SubjectKind::SourceTree)
        })
        .collect();

    if !selected.is_empty() && execution.is_empty() {
        bail!("none of the selected Rules apply to this Subject");
    }

    let mut diagnostics = Vec::new();

    for rule in &execution {
        let mut ctx = LintContext::new(&intent.subject, *rule);
        rule.run(&mut ctx);
        let mut rule_diagnostics = ctx.into_diagnostics();
        apply_severity_remaps(&mut rule_diagnostics, intent);
        diagnostics.extend(rule_diagnostics);
    }

    let outcome = outcome_for(&diagnostics, &intent.fail_on);
    Ok((diagnostics, outcome))
}

fn apply_severity_remaps(diagnostics: &mut [Diagnostic], intent: &CheckIntent) {
    for diagnostic in diagnostics {
        if let Some(remap) = intent.severity_remaps.get(&diagnostic.code) {
            diagnostic.severity = *remap;
        }
    }
}

pub(crate) fn outcome_for(diagnostics: &[Diagnostic], fail_on: &HashSet<Severity>) -> CheckOutcome {
    if diagnostics
        .iter()
        .any(|diagnostic| fail_on.contains(&diagnostic.severity))
    {
        CheckOutcome::PolicyFailure
    } else {
        CheckOutcome::Success
    }
}

pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let mut output = diagnostic.location.path.display().to_string();
    if let Some(line) = diagnostic.location.line {
        write!(output, ":{line}").expect("writing to string never fails");
        if let Some(column) = diagnostic.location.column {
            write!(output, ":{column}").expect("writing to string never fails");
        }
    }
    write!(
        output,
        ": {} [{}] {}",
        diagnostic.code, diagnostic.tag, diagnostic.message
    )
    .expect("writing to string never fails");
    output
}

/// Ruff-style end-of-run line: `All checks passed!` or `Found 1 warning.`
pub fn format_summary(diagnostics: &[Diagnostic]) -> String {
    if diagnostics.is_empty() {
        return "All checks passed!".to_string();
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    let mut pedantic = 0usize;
    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => infos += 1,
            Severity::Pedantic => pedantic += 1,
        }
    }

    let mut parts = Vec::new();
    push_count(&mut parts, errors, "error", "errors");
    push_count(&mut parts, warnings, "warning", "warnings");
    push_count(&mut parts, infos, "info", "infos");
    push_count(&mut parts, pedantic, "pedantic", "pedantic");
    format!("Found {}.", parts.join(", "))
}

fn push_count(parts: &mut Vec<String>, count: usize, singular: &str, plural: &str) {
    match count {
        0 => {}
        1 => parts.push(format!("1 {singular}")),
        n => parts.push(format!("{n} {plural}")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Context;

    use super::*;
    use crate::lint::diagnostic::Location;
    use crate::lint::intent::{CheckIntentInput, resolve_check_intent};
    use crate::lint::rule::{RuleMeta, Severity};
    use crate::lint::rules::DebmagicDummyTrigger;
    use crate::lint::selection::resolve_selected_codes;

    fn base_intent(subject: PathBuf) -> CheckIntent {
        resolve_check_intent(CheckIntentInput {
            fallback_dir: subject.clone(),
            subject: Some(subject),
            config_file: None,
            select: vec![],
            ignore: vec![],
            fail_on: vec![],
        })
        .expect("resolve check intent")
    }

    fn dummy_diagnostic(severity: Severity, message: &str) -> Diagnostic {
        Diagnostic::warn(message)
            .with_severity(severity)
            .with_code(DebmagicDummyTrigger::CODE)
            .with_tag(DebmagicDummyTrigger::TAG)
            .with_location(Location {
                path: PathBuf::from("debian/control"),
                line: None,
                column: None,
            })
    }

    #[test]
    fn fail_on_default_treats_warning_as_success() {
        let diagnostics = vec![dummy_diagnostic(Severity::Warning, "dummy Rule triggered")];
        assert_eq!(
            outcome_for(&diagnostics, &HashSet::from([Severity::Error])),
            CheckOutcome::Success
        );
    }

    #[test]
    fn fail_on_default_treats_error_as_policy_failure() {
        let diagnostics = vec![dummy_diagnostic(Severity::Error, "dummy Rule triggered")];
        assert_eq!(
            outcome_for(&diagnostics, &HashSet::from([Severity::Error])),
            CheckOutcome::PolicyFailure
        );
    }

    #[test]
    fn summary_all_checks_passed_when_empty() {
        assert_eq!(format_summary(&[]), "All checks passed!");
    }

    #[test]
    fn summary_counts_mixed_severities() {
        let diagnostics = vec![
            dummy_diagnostic(Severity::Error, "a"),
            dummy_diagnostic(Severity::Error, "b"),
            dummy_diagnostic(Severity::Warning, "c"),
        ];
        assert_eq!(format_summary(&diagnostics), "Found 2 errors, 1 warning.");
    }

    #[test]
    fn missing_debian_directory_errors() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-no-debian")?;
        let intent = base_intent(temp_dir);
        let error = run_check(&intent).unwrap_err();
        assert!(error.to_string().contains("debian"));
        Ok(())
    }

    #[test]
    fn binary_package_errors_unimplemented() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("debmagic-check-{}.deb", std::process::id()));
        fs::write(&path, b"")?;
        let intent = base_intent(path.clone());
        let error = run_check(&intent).unwrap_err();
        assert!(error.to_string().contains("Binary package"));
        assert!(error.to_string().contains("not implemented"));
        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn source_package_errors_unimplemented() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("debmagic-check-{}.dsc", std::process::id()));
        fs::write(&path, b"")?;
        let intent = base_intent(path.clone());
        let error = run_check(&intent).unwrap_err();
        assert!(error.to_string().contains("Source package"));
        assert!(error.to_string().contains("not implemented"));
        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn select_ln_runs_required_field_on_complete_control() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-ln-select")?;
        fs::create_dir_all(temp_dir.join("debian"))?;
        fs::write(
            temp_dir.join("debian/control"),
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n",
        )?;
        let selected = resolve_selected_codes(&["LN".to_string()], &[])?;
        assert_eq!(selected, vec![crate::lint::rules::RequiredField::CODE]);
        let intent = CheckIntent {
            subject: Subject::SourceTree(temp_dir),
            selected,
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_check(&intent)?;
        assert!(diagnostics.is_empty());
        assert_eq!(outcome, CheckOutcome::Success);
        Ok(())
    }

    fn tempfile_dir(prefix: &str) -> anyhow::Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("cleaning {}", dir.display()))?;
        }
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(dir)
    }
}
