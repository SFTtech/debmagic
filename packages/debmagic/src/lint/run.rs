use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use rayon::prelude::*;

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

/// One lint run: diagnostics plus the figures for the finished line.
#[derive(Debug)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
    pub outcome: LintOutcome,
    pub rules: usize,
    pub elapsed: Duration,
    /// Size of the thread pool the run used, for running Rules and for
    /// walking and classifying files.
    pub threads: usize,
}

pub fn run_lint(intent: &LintIntent) -> anyhow::Result<LintResult> {
    let started = Instant::now();
    let threads = worker_threads(intent.threads);
    // The whole run happens inside this pool, so everything that
    // parallelizes (Rules, file classification) follows `--threads`.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|index| format!("debmagic-lint-{index}"))
        .build()
        .context("starting the lint thread pool failed")?;
    let mut result = pool.install(|| match &intent.subject {
        Subject::SourcePackage(path) => run_source_package_lint(intent, path),
        Subject::BinaryPackage(path) => run_binary_package_lint(intent, path),
        Subject::SourceTree(path) => run_source_tree_lint(intent, path),
    })?;
    result.elapsed = started.elapsed();
    result.threads = threads;
    Ok(result)
}

fn run_source_package_lint(intent: &LintIntent, path: &Path) -> anyhow::Result<LintResult> {
    let data = SourcePackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::source_rules(), |rule| {
        let mut ctx = SourceContext::bind_package(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_binary_package_lint(intent: &LintIntent, path: &Path) -> anyhow::Result<LintResult> {
    let data = BinaryPackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::binary_package_rules(), |rule| {
        let mut ctx = BinaryPackageContext::bind(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_source_tree_lint(intent: &LintIntent, source_tree: &Path) -> anyhow::Result<LintResult> {
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

/// Worker threads for a run: the requested count (0 counts as unset), or the
/// machine's available parallelism. At least 1.
fn worker_threads(requested: Option<usize>) -> usize {
    requested
        .filter(|&threads| threads > 0)
        .or_else(|| std::thread::available_parallelism().ok().map(Into::into))
        .unwrap_or(1)
}

/// Run every selected Rule on the current rayon pool. Rules are independent, so
/// they run in parallel;
/// diagnostics are collected in Rule order, so output does not depend on the
/// thread count. If several Rules fail, the first in Rule order is reported.
fn execute_rules<R: RuleAccess + ?Sized>(
    intent: &LintIntent,
    rules: &[&R],
    run_rule: impl Fn(&R) -> anyhow::Result<Vec<Diagnostic>> + Sync,
) -> anyhow::Result<LintResult> {
    let selected = intent.selected.iter().copied().collect::<HashSet<_>>();
    let execution: Vec<_> = rules
        .iter()
        .copied()
        .filter(|rule| selected.contains(&rule.code()))
        .collect();

    if !selected.is_empty() && execution.is_empty() {
        bail!("none of the selected Rules apply to this Subject");
    }

    let rules_run = execution.len();

    let run_one = |rule: &&R| -> anyhow::Result<Vec<Diagnostic>> {
        let mut rule_diagnostics = run_rule(rule)?;
        apply_severity_remaps(&mut rule_diagnostics, intent);
        Ok(rule_diagnostics)
    };

    let per_rule: Vec<anyhow::Result<Vec<Diagnostic>>> =
        execution.par_iter().map(run_one).collect();

    let mut diagnostics = Vec::new();
    for rule_diagnostics in per_rule {
        diagnostics.extend(rule_diagnostics?);
    }

    let outcome = outcome_for(&diagnostics, &intent.fail_on);
    Ok(LintResult {
        diagnostics,
        outcome,
        rules: rules_run,
        elapsed: Duration::ZERO,
        threads: 0,
    })
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

/// Oxlint-style end-of-run line, without a file count.
///
/// Under a second the duration is whole milliseconds (`786ms`); otherwise
/// seconds with one decimal (`1.0s`). Rule and thread counts use a singular
/// word at 1.
pub fn format_finished(elapsed: Duration, rules: usize, threads: usize) -> String {
    let ms = elapsed.as_millis();
    let time = if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", elapsed.as_secs_f64())
    };
    format!(
        "Finished in {time} with {rules} {} using {threads} {}.",
        plural(rules, "rule", "rules"),
        plural(threads, "thread", "threads"),
    )
}

fn plural(count: usize, singular: &'static str, plural_word: &'static str) -> &'static str {
    if count == 1 { singular } else { plural_word }
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
    use std::time::Duration;

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
            threads: None,
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
            threads: None,
        };
        let LintResult { diagnostics, .. } = run_lint(&intent)?;
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
    fn finished_line_matches_oxlint_shape() {
        assert_eq!(
            format_finished(Duration::from_millis(786), 179, 32),
            "Finished in 786ms with 179 rules using 32 threads."
        );
        assert_eq!(
            format_finished(Duration::from_secs(1), 1, 1),
            "Finished in 1.0s with 1 rule using 1 thread."
        );
        assert_eq!(
            format_finished(Duration::from_millis(1500), 2, 1),
            "Finished in 1.5s with 2 rules using 1 thread."
        );
    }

    #[test]
    fn run_counts_executed_rules() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-finished")?;
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
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: Some(1),
        };
        let result = run_lint(&intent)?;
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.rules, 1);
        assert_eq!(result.threads, 1);
        assert!(
            format_finished(result.elapsed, result.rules, result.threads)
                .ends_with(" with 1 rule using 1 thread.")
        );
        Ok(())
    }

    #[test]
    fn worker_threads_uses_request_or_available_parallelism() {
        assert_eq!(worker_threads(Some(8)), 8);
        assert_eq!(worker_threads(Some(1)), 1);
        assert!(worker_threads(Some(0)) >= 1);
        assert!(worker_threads(None) >= 1);
    }

    #[test]
    fn lint_data_is_shareable_across_threads() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SourceTreeData>();
        assert_sync::<SourcePackageData>();
        assert_sync::<BinaryPackageData>();
    }

    #[test]
    fn diagnostics_do_not_depend_on_thread_count() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-threads")?;
        fs::create_dir_all(temp_dir.join("debian"))?;
        // Missing fields and no changelog, so several Rules report findings.
        fs::write(
            temp_dir.join("debian/control"),
            "Maintainer: Example <ex@example.com>\n\
             \n\
             Package: example\n\
             Architecture: all\n",
        )?;
        let selected = resolve_selected_codes(&["LN".to_string()], &[])?;
        let lint_with = |threads: Option<usize>| -> anyhow::Result<Vec<String>> {
            let intent = LintIntent {
                subject: Subject::SourceTree(temp_dir.clone()),
                selected: selected.clone(),
                fail_on: HashSet::from([Severity::Error]),
                severity_remaps: HashMap::new(),
                threads,
            };
            let result = run_lint(&intent)?;
            assert_eq!(result.threads, worker_threads(threads));
            Ok(result.diagnostics.iter().map(format_diagnostic).collect())
        };

        let serial = lint_with(Some(1))?;
        assert!(!serial.is_empty(), "fixture should produce diagnostics");
        for threads in [Some(2), Some(8), None] {
            assert_eq!(lint_with(threads)?, serial, "threads: {threads:?}");
        }
        Ok(())
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
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
            threads: None,
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
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
                crate::lint::rules::PackageContainsUnsafeSymlink::CODE,
                crate::lint::rules::EmptyBinaryPackage::CODE,
                crate::lint::rules::SuperfluousFilePattern::CODE,
                crate::lint::rules::NoDep5Copyright::CODE,
                crate::lint::rules::NoCopyrightFile::CODE,
                crate::lint::rules::FileInUnusualDir::CODE,
                crate::lint::rules::NoChangelog::CODE,
                crate::lint::rules::DebianChangelogFileMissingOrWrongName::CODE,
                crate::lint::rules::MaintainerScriptEmpty::CODE,
                crate::lint::rules::SourceContainsDebianSubstvars::CODE,
                crate::lint::rules::CustomCompressionInDebianRules::CODE,
                crate::lint::rules::DebianRulesIgnoresMakeCleanError::CODE,
                crate::lint::rules::NationalEncoding::CODE,
                crate::lint::rules::PackageUsesDeprecatedDebhelperCompatVersion::CODE,
                crate::lint::rules::UsesDebhelperCompatFile::CODE,
                crate::lint::rules::NoHumanMaintainers::CODE,
                crate::lint::rules::DmUploadAllowedIsObsolete::CODE,
                crate::lint::rules::VcsObsoleteInDebianInfrastructure::CODE,
                crate::lint::rules::ChangelogFileNotCompressed::CODE,
            ]
        );
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected,
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
        };
        let LintResult {
            diagnostics,
            outcome,
            ..
        } = run_lint(&intent)?;
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
