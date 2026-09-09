use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;

use anyhow::bail;

use super::context::{
    BinaryPackageContext, BinaryPackageData, SourceContext, SourcePackageData, SourceTreeData,
};
use super::diagnostic::Diagnostic;
use super::intent::LintIntent;
use super::registry;
use super::rule::{RuleAccess, Severity};
use super::subject::Subject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintOutcome {
    Success,
    PolicyFailure,
}

pub fn run_lint(intent: &LintIntent) -> anyhow::Result<(Vec<Diagnostic>, LintOutcome)> {
    match &intent.subject {
        Subject::SourcePackage(path) => run_source_package_lint(intent, path),
        Subject::BinaryPackage(path) => run_binary_package_lint(intent, path),
        Subject::SourceTree(path) => run_source_tree_lint(intent, path),
    }
}

fn run_source_package_lint(
    intent: &LintIntent,
    path: &Path,
) -> anyhow::Result<(Vec<Diagnostic>, LintOutcome)> {
    let data = SourcePackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::source_rules(), |rule| {
        let mut ctx = SourceContext::bind_package(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_binary_package_lint(
    intent: &LintIntent,
    path: &Path,
) -> anyhow::Result<(Vec<Diagnostic>, LintOutcome)> {
    let data = BinaryPackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::binary_package_rules(), |rule| {
        let mut ctx = BinaryPackageContext::bind(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_source_tree_lint(
    intent: &LintIntent,
    source_tree: &Path,
) -> anyhow::Result<(Vec<Diagnostic>, LintOutcome)> {
    let debian_dir = source_tree.join("debian");
    if !debian_dir.is_dir() {
        bail!(
            "source directory {} requires a debian/ directory",
            source_tree.display()
        );
    }

    let data = SourceTreeData::new(source_tree.to_path_buf());
    execute_rules(intent, registry::source_rules(), |rule| {
        let mut ctx = SourceContext::bind_tree(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn execute_rules<R: RuleAccess + ?Sized>(
    intent: &LintIntent,
    rules: &[&R],
    mut run_rule: impl FnMut(&R) -> anyhow::Result<Vec<Diagnostic>>,
) -> anyhow::Result<(Vec<Diagnostic>, LintOutcome)> {
    let selected = intent.selected.iter().copied().collect::<HashSet<_>>();
    let execution: Vec<_> = rules
        .iter()
        .copied()
        .filter(|rule| selected.contains(&rule.code()))
        .collect();

    if !selected.is_empty() && execution.is_empty() {
        bail!("none of the selected Rules apply to this Subject");
    }

    let mut diagnostics = Vec::new();

    for rule in execution {
        let mut rule_diagnostics = run_rule(rule)?;
        apply_severity_remaps(&mut rule_diagnostics, intent);
        diagnostics.extend(rule_diagnostics);
    }

    let outcome = outcome_for(&diagnostics, &intent.fail_on);
    Ok((diagnostics, outcome))
}

fn apply_severity_remaps(diagnostics: &mut [Diagnostic], intent: &LintIntent) {
    for diagnostic in diagnostics {
        if let Some(remap) = intent.severity_remaps.get(&diagnostic.code) {
            diagnostic.severity = *remap;
        }
    }
}

pub(crate) fn outcome_for(diagnostics: &[Diagnostic], fail_on: &HashSet<Severity>) -> LintOutcome {
    if diagnostics
        .iter()
        .any(|diagnostic| fail_on.contains(&diagnostic.severity))
    {
        LintOutcome::PolicyFailure
    } else {
        LintOutcome::Success
    }
}

pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let mut output = format!("{}:", diagnostic.severity.letter());
    if !diagnostic.package.is_empty() {
        write!(output, " {}", diagnostic.package).expect("writing to string never fails");
    }
    if let Some(processable_type) = diagnostic.processable_type {
        write!(output, " ({processable_type})").expect("writing to string never fails");
    }
    write!(output, ": {} ({})", diagnostic.tag, diagnostic.code)
        .expect("writing to string never fails");
    if !diagnostic.message.is_empty() {
        write!(output, " {}", diagnostic.message).expect("writing to string never fails");
    }
    if !diagnostic.location.path.as_os_str().is_empty() {
        let mut pointer = diagnostic.location.path.display().to_string();
        if let Some(line) = diagnostic.location.line {
            write!(pointer, ":{line}").expect("writing to string never fails");
            if let Some(column) = diagnostic.location.column {
                write!(pointer, ":{column}").expect("writing to string never fails");
            }
        }
        write!(output, " [{pointer}]").expect("writing to string never fails");
    }
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
    use crate::lint::diagnostic::{Location, ProcessableType};
    use crate::lint::intent::{LintIntentInput, resolve_lint_intent};
    use crate::lint::rule::{RuleMeta, Severity};
    use crate::lint::rules::{DebmagicDummyTrigger, RequiredField};
    use crate::lint::selection::resolve_selected_codes;

    fn base_intent(subject: PathBuf) -> LintIntent {
        resolve_lint_intent(LintIntentInput {
            fallback_dir: subject.clone(),
            subject: Some(subject),
            config_file: None,
            select: vec![],
            ignore: vec![],
            fail_on: vec![],
        })
        .expect("resolve lint intent")
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
    fn format_diagnostic_mirrors_lintian_pointed_hint() {
        let diagnostic = Diagnostic::error("(in section for generic-empty) Description")
            .with_code(RequiredField::CODE)
            .with_tag(RequiredField::TAG)
            .with_package("generic-empty")
            .with_processable_type(ProcessableType::Source)
            .with_location(Location {
                path: PathBuf::from("debian/control"),
                line: Some(4),
                column: None,
            });
        assert_eq!(
            format_diagnostic(&diagnostic),
            "E: generic-empty (source): required-field (LN0001) (in section for generic-empty) Description [debian/control:4]"
        );
    }

    #[test]
    fn diagnostic_package_falls_back_to_changelog() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-changelog-package")?;
        fs::create_dir_all(temp_dir.join("debian"))?;
        fs::write(
            temp_dir.join("debian/changelog"),
            "generic-empty (1.0) unstable; urgency=low\n\n  * test\n\n -- a <a@localhost>  Tue, 30 Dec 2008 17:34:02 -0800\n",
        )?;
        fs::write(
            temp_dir.join("debian/control"),
            "Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: generic-empty\n\
             Architecture: all\n\
             Description: example\n extra\n",
        )?;
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, _) = run_lint(&intent)?;
        assert!(
            diagnostics.iter().any(|diagnostic| {
                format_diagnostic(diagnostic)
                    .starts_with("E: generic-empty (source): required-field (LN0001)")
            }),
            "expected changelog package in diagnostic output, got: {:?}",
            diagnostics
                .iter()
                .map(format_diagnostic)
                .collect::<Vec<_>>(),
        );
        Ok(())
    }

    #[test]
    fn fail_on_default_treats_warning_as_success() {
        let diagnostics = vec![dummy_diagnostic(Severity::Warning, "dummy Rule triggered")];
        assert_eq!(
            outcome_for(&diagnostics, &HashSet::from([Severity::Error])),
            LintOutcome::Success
        );
    }

    #[test]
    fn fail_on_default_treats_error_as_policy_failure() {
        let diagnostics = vec![dummy_diagnostic(Severity::Error, "dummy Rule triggered")];
        assert_eq!(
            outcome_for(&diagnostics, &HashSet::from([Severity::Error])),
            LintOutcome::PolicyFailure
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
        let error = run_lint(&intent).unwrap_err();
        assert!(error.to_string().contains("debian"));
        Ok(())
    }

    #[test]
    fn malformed_binary_package_is_runtime_failure() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("debmagic-check-{}.deb", std::process::id()));
        fs::write(&path, b"")?;
        let intent = base_intent(path.clone());
        let error = run_lint(&intent).unwrap_err();
        assert!(
            error.to_string().contains("control.tar") || error.to_string().contains("ar"),
            "{error}"
        );
        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn binary_package_required_field_uses_installation_control() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let path = dir.path().join("fields-general-missing.deb");
        crate::lint::binary_package_reader::write_test_deb(
            &path,
            "Section: devel\nPriority: optional\nDescription: missing fields\n extra\n",
        )?;
        let intent = LintIntent {
            subject: Subject::BinaryPackage(std::path::absolute(&path)?),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert_eq!(outcome, LintOutcome::PolicyFailure);
        let rendered: Vec<_> = diagnostics.iter().map(format_diagnostic).collect();
        assert!(
            rendered.iter().any(|line| {
                line.contains("fields-general-missing (binary): required-field (LN0001) fields-general-missing.deb Package")
            }),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("fields-general-missing.deb Version")),
            "{rendered:?}"
        );
        Ok(())
    }

    #[test]
    fn unstripped_elf_in_binary_package_is_reported() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let path = dir.path().join("example.deb");
        let elf = std::fs::read(std::env::current_exe()?)?;
        crate::lint::binary_package_reader::write_test_deb_with_files(
            &path,
            &[(
                "control",
                b"Package: example\n\
                  Version: 1.0\n\
                  Architecture: all\n\
                  Maintainer: Example <ex@example.com>\n\
                  Description: example\n extra\n",
            )],
            &[("usr/bin/app", elf.as_slice())],
        )?;
        let intent = LintIntent {
            subject: Subject::BinaryPackage(std::path::absolute(&path)?),
            selected: vec![crate::lint::rules::UnstrippedBinaryOrObject::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert_eq!(outcome, LintOutcome::PolicyFailure);
        let rendered: Vec<_> = diagnostics.iter().map(format_diagnostic).collect();
        assert!(
            rendered.iter().any(|line| {
                line.contains("example (binary): unstripped-binary-or-object (LN0005)")
                    && line.contains("[usr/bin/app]")
            }),
            "{rendered:?}"
        );
        Ok(())
    }

    #[test]
    fn complete_binary_package_has_no_required_field() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let path = dir.path().join("example.deb");
        crate::lint::binary_package_reader::write_test_deb(
            &path,
            "Package: example\n\
             Version: 1.0\n\
             Architecture: all\n\
             Maintainer: Example <ex@example.com>\n\
             Description: example\n extra\n",
        )?;
        let intent = LintIntent {
            subject: Subject::BinaryPackage(std::path::absolute(&path)?),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(outcome, LintOutcome::Success);
        Ok(())
    }

    #[test]
    fn select_source_tree_only_rule_on_binary_package_none_apply() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let path = dir.path().join("example.deb");
        crate::lint::binary_package_reader::write_test_deb(
            &path,
            "Package: example\n\
             Version: 1.0\n\
             Architecture: all\n\
             Maintainer: Example <ex@example.com>\n\
             Description: example\n extra\n",
        )?;
        let intent = LintIntent {
            subject: Subject::BinaryPackage(std::path::absolute(&path)?),
            selected: vec![crate::lint::rules::SyntaxErrorInDebianChangelog::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let error = run_lint(&intent).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("none of the selected Rules apply to this Subject"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn source_package_empty_dsc_is_runtime_failure() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("debmagic-check-{}.dsc", std::process::id()));
        fs::write(&path, b"")?;
        let intent = base_intent(path.clone());
        let error = run_lint(&intent).unwrap_err();
        assert!(
            error.to_string().contains("Deb822")
                || error.to_string().contains("paragraph")
                || error.to_string().contains("files")
                || error.to_string().contains("readable .dsc"),
            "{error}"
        );
        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn source_package_native_dsc_runs_windows_binary() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let exe = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/lintian-parity/checks/cruft/cruft-general-upstream/fake-win32-bin.exe"
        ));
        let dsc = crate::lint::source_package::write_test_native_dsc(
            dir.path(),
            "example",
            &[("bin/app.exe", exe.as_slice())],
        )?;
        let intent = LintIntent {
            subject: Subject::SourcePackage(std::path::absolute(&dsc)?),
            selected: vec![crate::lint::rules::SourceContainsPrebuiltWindowsBinary::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert_eq!(outcome, LintOutcome::Success);
        let rendered: Vec<_> = diagnostics.iter().map(format_diagnostic).collect();
        assert!(
            rendered.iter().any(|line| {
                line.contains("example (source): source-contains-prebuilt-windows-binary (LN0004)")
                    && line.contains("bin/app.exe")
            }),
            "{rendered:?}"
        );
        Ok(())
    }

    #[test]
    fn select_source_rule_on_source_package_applies() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let dsc = crate::lint::source_package::write_test_native_dsc(dir.path(), "example", &[])?;
        let intent = LintIntent {
            subject: Subject::SourcePackage(std::path::absolute(&dsc)?),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(outcome, LintOutcome::Success);
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
        assert_eq!(
            selected,
            vec![
                crate::lint::rules::RequiredField::CODE,
                crate::lint::rules::SyntaxErrorInDebianChangelog::CODE,
                crate::lint::rules::DebianRulesMissingRequiredTarget::CODE,
                crate::lint::rules::SourceContainsPrebuiltWindowsBinary::CODE,
                crate::lint::rules::UnstrippedBinaryOrObject::CODE,
            ]
        );
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected,
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
        };
        let (diagnostics, outcome) = run_lint(&intent)?;
        assert!(diagnostics.is_empty());
        assert_eq!(outcome, LintOutcome::Success);
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
