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
use super::rule::{Code, RuleAccess, Severity, Tag};
use super::subject::Subject;
use super::timing::{self, TimingSession};

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
    /// Present when the run was asked for a timings report.
    pub timings: Option<LintTimings>,
}

/// One Rule's time, excluding initialization shared with other Rules.
#[derive(Debug, Clone, Copy)]
pub struct RuleTiming {
    pub code: Code,
    pub tag: Tag,
    pub elapsed: Duration,
}

/// Bringup, shared initialization, and per-Rule time for one lint run.
///
/// `bringup` is wall time before any Rule runs (thread pool and opening the
/// Subject). `shared` is time spent building data every Rule shares — File
/// trees, Debian control, Debian changelog, tarball indexes, and other facts
/// computed once per Subject — counted once, not on the Rule that happened to
/// trigger it. A Rule's `elapsed` excludes time that Rule spent waiting on or
/// performing that shared work.
#[derive(Debug, Clone)]
pub struct LintTimings {
    pub bringup: Duration,
    pub shared: Duration,
    pub rules: Vec<RuleTiming>,
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
        Subject::SourcePackage(path) => run_source_package_lint(intent, path, started),
        Subject::BinaryPackage(path) => run_binary_package_lint(intent, path, started),
        Subject::SourceTree(path) => run_source_tree_lint(intent, path, started),
    })?;
    result.elapsed = started.elapsed();
    result.threads = threads;
    Ok(result)
}

fn run_source_package_lint(
    intent: &LintIntent,
    path: &Path,
    started: Instant,
) -> anyhow::Result<LintResult> {
    let data = SourcePackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::source_rules(), started, |rule| {
        let mut ctx = SourceContext::bind_package(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_binary_package_lint(
    intent: &LintIntent,
    path: &Path,
    started: Instant,
) -> anyhow::Result<LintResult> {
    let data = BinaryPackageData::new(path.to_path_buf())?;
    execute_rules(intent, registry::binary_package_rules(), started, |rule| {
        let mut ctx = BinaryPackageContext::bind(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })
}

fn run_source_tree_lint(
    intent: &LintIntent,
    source_tree: &Path,
    started: Instant,
) -> anyhow::Result<LintResult> {
    let debian_dir = source_tree.join("debian");
    if !debian_dir.is_dir() {
        bail!(
            "source directory {} requires a debian/ directory",
            source_tree.display()
        );
    }

    let data = SourceTreeData::new(source_tree.to_path_buf());
    execute_rules(intent, registry::source_rules(), started, |rule| {
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

struct RuleProduct {
    diagnostics: Vec<Diagnostic>,
    timing: RuleTiming,
}

/// Run every selected Rule on the current rayon pool. Rules are independent, so
/// they run in parallel;
/// diagnostics are collected in Rule order, so output does not depend on the
/// thread count. If several Rules fail, the first in Rule order is reported.
///
/// `started` is the beginning of the lint run. Time up to the moment Rules
/// launch is bringup. Shared initialization that happens inside a Rule is
/// subtracted from that Rule and reported once.
fn execute_rules<R: RuleAccess + ?Sized>(
    intent: &LintIntent,
    rules: &[&R],
    started: Instant,
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
    // Installed before bringup is sampled so pool broadcast is bringup, and
    // before Rules run so their shared initialization is visible on every worker.
    let session = intent.timings.then(TimingSession::install_on_current_pool);
    let bringup = started.elapsed();

    let run_one = |rule: &&R| -> anyhow::Result<RuleProduct> {
        let start = Instant::now();
        let waited_before = timing::waited_on_this_thread();
        let mut diagnostics = run_rule(rule)?;
        apply_severity_remaps(&mut diagnostics, intent);
        let waited = timing::waited_on_this_thread().saturating_sub(waited_before);
        Ok(RuleProduct {
            diagnostics,
            timing: RuleTiming {
                code: rule.code(),
                tag: rule.tag(),
                elapsed: start.elapsed().saturating_sub(waited),
            },
        })
    };

    let per_rule: Vec<anyhow::Result<RuleProduct>> = execution.par_iter().map(run_one).collect();
    let shared = session
        .as_ref()
        .map(TimingSession::shared)
        .unwrap_or_default();
    drop(session);

    let mut diagnostics = Vec::new();
    let mut rule_timings = Vec::with_capacity(per_rule.len());
    for product in per_rule {
        let product = product?;
        diagnostics.extend(product.diagnostics);
        rule_timings.push(product.timing);
    }

    let outcome = outcome_for(&diagnostics, &intent.fail_on);
    Ok(LintResult {
        diagnostics,
        outcome,
        rules: rules_run,
        elapsed: Duration::ZERO,
        threads: 0,
        timings: intent.timings.then_some(LintTimings {
            bringup,
            shared,
            rules: rule_timings,
        }),
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

/// Timings report: shared initialization, then each Rule slowest first.
///
/// Initialization is `bringup` (wall time before any Rule runs) and `shared`
/// (data computed once for every Rule). A Rule's time excludes shared
/// initialization that Rule waited on or performed.
pub fn format_timings(timings: &LintTimings) -> String {
    let mut rules = timings.rules.clone();
    rules.sort_by(|left, right| {
        right
            .elapsed
            .cmp(&left.elapsed)
            .then_with(|| left.code.cmp(&right.code))
    });
    let rule_labels: Vec<String> = rules
        .iter()
        .map(|rule| format!("{}  {}", rule.code, rule.tag))
        .collect();
    let bringup = format_timing_duration(timings.bringup);
    let shared = format_timing_duration(timings.shared);
    let rule_times: Vec<String> = rules
        .iter()
        .map(|rule| format_timing_duration(rule.elapsed))
        .collect();
    let width = rule_labels
        .iter()
        .map(|label| 2 + label.len())
        .chain([2 + "bringup".len(), 2 + "shared".len()])
        .max()
        .unwrap_or(0);
    let time_width = [bringup.len(), shared.len()]
        .into_iter()
        .chain(rule_times.iter().map(String::len))
        .max()
        .unwrap_or(0);

    let mut lines = vec![
        "Initialization".to_string(),
        timing_row(2, "bringup", &bringup, width, time_width),
        timing_row(2, "shared", &shared, width, time_width),
    ];
    if !rules.is_empty() {
        lines.push(String::new());
        lines.push("Rules".to_string());
        for (label, time) in rule_labels.iter().zip(&rule_times) {
            lines.push(timing_row(2, label, time, width, time_width));
        }
    }
    lines.join("\n")
}

fn format_timing_duration(elapsed: Duration) -> String {
    if elapsed.as_secs() >= 1 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        format!("{:.1}ms", elapsed.as_secs_f64() * 1000.0)
    }
}

fn timing_row(indent: usize, label: &str, time: &str, width: usize, time_width: usize) -> String {
    let mut line = " ".repeat(indent);
    line.push_str(label);
    line.push_str(&" ".repeat(width.saturating_sub(line.len())));
    line.push_str("  ");
    line.push_str(&" ".repeat(time_width.saturating_sub(time.len())));
    line.push_str(time);
    line
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
            timings: false,
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
            timings: false,
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
    fn timing_duration_switches_to_seconds_at_one_second() {
        assert_eq!(format_timing_duration(Duration::ZERO), "0.0ms");
        assert_eq!(format_timing_duration(Duration::from_micros(1500)), "1.5ms");
        assert_eq!(
            format_timing_duration(Duration::from_millis(999)),
            "999.0ms"
        );
        assert_eq!(format_timing_duration(Duration::from_secs(1)), "1.0s");
        assert_eq!(format_timing_duration(Duration::from_millis(1500)), "1.5s");
    }

    #[test]
    fn timings_report_splits_initialization_and_rules() {
        let timings = LintTimings {
            bringup: Duration::from_millis(12),
            shared: Duration::from_millis(340),
            rules: vec![
                RuleTiming {
                    code: RequiredField::CODE,
                    tag: RequiredField::TAG,
                    elapsed: Duration::from_millis(1),
                },
                RuleTiming {
                    code: DebmagicDummyTrigger::CODE,
                    tag: DebmagicDummyTrigger::TAG,
                    elapsed: Duration::from_millis(20),
                },
            ],
        };
        assert_eq!(
            format_timings(&timings),
            "\
Initialization
  bringup                          12.0ms
  shared                          340.0ms

Rules
  DM0001  debmagic-dummy-trigger   20.0ms
  LN0001  required-field            1.0ms"
        );
    }

    #[test]
    fn timings_exclude_shared_initialization_from_rules() -> anyhow::Result<()> {
        let temp_dir = tempfile_dir("debmagic-check-timings")?;
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
        for index in 0..2_000 {
            fs::write(temp_dir.join(format!("f{index}")), b"x")?;
        }
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected: vec![DebmagicDummyTrigger::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: Some(1),
            timings: true,
        };
        let result = run_lint(&intent)?;
        let timings = result.timings.expect("timings");
        assert_eq!(timings.rules.len(), 1);
        assert_eq!(timings.rules[0].code, DebmagicDummyTrigger::CODE);
        assert!(timings.bringup > Duration::ZERO, "{:?}", timings.bringup);
        assert!(
            timings.shared > timings.rules[0].elapsed,
            "shared initialization was charged to the rule: shared {:?} rule {:?}",
            timings.shared,
            timings.rules[0].elapsed
        );
        let rendered = format_timings(&timings);
        assert!(rendered.starts_with("Initialization\n"), "{rendered}");
        assert!(rendered.contains("\n  bringup"), "{rendered}");
        assert!(rendered.contains("\n  shared"), "{rendered}");
        assert!(rendered.contains("\n\nRules\n"), "{rendered}");
        assert!(
            rendered.contains("DM0001  debmagic-dummy-trigger"),
            "{rendered}"
        );
        Ok(())
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
            timings: false,
        };
        let result = run_lint(&intent)?;
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.rules, 1);
        assert_eq!(result.threads, 1);
        assert!(result.timings.is_none());
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
                timings: false,
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
            timings: false,
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
            timings: false,
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
            timings: false,
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
            timings: false,
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
            timings: false,
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
            timings: false,
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
        fs::write(
            temp_dir.join("debian/copyright"),
            "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
             \n\
             Files: *\n\
             Copyright: 2024 Example\n\
             License: Expat\n",
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
                crate::lint::rules::NonStandardToplevelDir::CODE,
                crate::lint::rules::LatestDebianChangelogEntryWithoutNewDate::CODE,
                crate::lint::rules::PackageRelationWithSelf::CODE,
                crate::lint::rules::MalformedDmUploadAllowed::CODE,
                crate::lint::rules::SymlinkContainsSpuriousSegments::CODE,
                crate::lint::rules::SymlinkIsSelfRecursive::CODE,
                crate::lint::rules::LengthySymlink::CODE,
                crate::lint::rules::WrongPackageMailAlias::CODE,
                crate::lint::rules::DebianChangelogFileIsASymlink::CODE,
                crate::lint::rules::PackageContainsVcsControlDir::CODE,
                crate::lint::rules::DirOrFileInSrv::CODE,
                crate::lint::rules::DirOrFileInMnt::CODE,
                crate::lint::rules::ExtendedDescriptionIsEmpty::CODE,
                crate::lint::rules::DirOrFileInHome::CODE,
                crate::lint::rules::DirOrFileInTmp::CODE,
                crate::lint::rules::DirOrFileInOpt::CODE,
                crate::lint::rules::DirOrFileInVarWww::CODE,
                crate::lint::rules::DirOrFileInVarRun::CODE,
                crate::lint::rules::DirOrFileInVarLock::CODE,
                crate::lint::rules::DirOrFileInRun::CODE,
                crate::lint::rules::DirOrFileInEtcOpt::CODE,
                crate::lint::rules::DirInUsrLocal::CODE,
                crate::lint::rules::FileInUsrLocal::CODE,
                crate::lint::rules::FileDirectlyInUsrShare::CODE,
                crate::lint::rules::SubdirInUsrBin::CODE,
                crate::lint::rules::DescriptionSynopsisIsEmpty::CODE,
                crate::lint::rules::DescriptionStartsWithPackageName::CODE,
                crate::lint::rules::DescriptionSynopsisStartsWithArticle::CODE,
                crate::lint::rules::DescriptionTooShort::CODE,
                crate::lint::rules::DescriptionIsPkgName::CODE,
                crate::lint::rules::SynopsisTooLong::CODE,
                crate::lint::rules::ExtendedDescriptionLineTooLong::CODE,
                crate::lint::rules::SvnCommitFileInPackage::CODE,
                crate::lint::rules::FsstndDirInUsr::CODE,
                crate::lint::rules::NonStandardDirInUsr::CODE,
                crate::lint::rules::FsstndDirInVar::CODE,
                crate::lint::rules::NonStandardDirInVar::CODE,
                crate::lint::rules::AliasedLocation::CODE,
                crate::lint::rules::UseOfCompatSymlink::CODE,
                crate::lint::rules::AbsoluteSymlinkInTopLevelFolder::CODE,
                crate::lint::rules::FileNameEndsInWhitespace::CODE,
                crate::lint::rules::SourceContainsCmakeCacheFile::CODE,
                crate::lint::rules::FileNameContainsWildcardCharacter::CODE,
                crate::lint::rules::RelativeSymlink::CODE,
                crate::lint::rules::SymlinkEndsWithSlash::CODE,
                crate::lint::rules::SymlinkHasDoubleSlash::CODE,
                crate::lint::rules::SymlinkHasTooManyUpSegments::CODE,
                crate::lint::rules::PackageContainsPythonDotDirectory::CODE,
                crate::lint::rules::PackageContainsPythonHypothesisExample::CODE,
                crate::lint::rules::PackageContainsSassCacheDirectory::CODE,
                crate::lint::rules::CompressedSymlinkWithWrongExt::CODE,
                crate::lint::rules::HyphenFile::CODE,
                crate::lint::rules::FileNameInPathIsNotAscii::CODE,
                crate::lint::rules::PackageContainsCompiledGlibSchema::CODE,
                crate::lint::rules::StarFile::CODE,
                crate::lint::rules::PackageContainsFileInEtcSkel::CODE,
                crate::lint::rules::PackageContainsFileInUsrShareHal::CODE,
                crate::lint::rules::PackageContainsIconCacheInGenericDir::CODE,
                crate::lint::rules::PackageContainsPythonCoverageFile::CODE,
                crate::lint::rules::PackageContainsPythonDoctreeFile::CODE,
                crate::lint::rules::PackageContainsPythonHeaderInIncorrectDirectory::CODE,
                crate::lint::rules::PackageContainsPythonTestsInGlobalNamespace::CODE,
                crate::lint::rules::PackageContainsEslintConfigFile::CODE,
                crate::lint::rules::PackageContainsNpmIgnoreFile::CODE,
                crate::lint::rules::MissingDebianWatchFileStandard::CODE,
                crate::lint::rules::ObsoleteDebianWatchFileStandard::CODE,
                crate::lint::rules::UnknownDebianWatchFileStandard::CODE,
                crate::lint::rules::MultipleDebianWatchFileStandards::CODE,
                crate::lint::rules::AncientPythonVersionField::CODE,
                crate::lint::rules::DebianWatchLineInvalid::CODE,
                crate::lint::rules::DebianWatchLacksSourceforgeRedirector::CODE,
                crate::lint::rules::DebianWatchFileUsesDeprecatedGithubredir::CODE,
                crate::lint::rules::DebianWatchFileUsesDeprecatedSfRedirectorMethod::CODE,
                crate::lint::rules::DebianWatchNotManglingVersion::CODE,
                crate::lint::rules::DebianWatchFileSpecifiesWrongUpstreamVersion::CODE,
                crate::lint::rules::DebianWatchFileSpecifiesOldUpstreamVersion::CODE,
                crate::lint::rules::DebianWatchFilePubkeyFileIsMissing::CODE,
                crate::lint::rules::DebianWatchCouldVerifyDownload::CODE,
                crate::lint::rules::MalformedPythonVersion::CODE,
                crate::lint::rules::PythonVersionCurrentIsDeprecated::CODE,
                crate::lint::rules::SourcePackageEncodesPythonVersion::CODE,
                crate::lint::rules::BogusMailHost::CODE,
                crate::lint::rules::DependsOnObsoletePackage::CODE,
                crate::lint::rules::DependsOnOldEmacs::CODE,
                crate::lint::rules::NotBinnmuableAllDependsAny::CODE,
                crate::lint::rules::NotBinnmuableAnyDependsAny::CODE,
                crate::lint::rules::NotBinnmuableAnyDependsAll::CODE,
                crate::lint::rules::OldFsfAddressInCopyrightFile::CODE,
                crate::lint::rules::CopyrightRefersToOldDirectory::CODE,
                crate::lint::rules::CopyrightRefersToDeprecatedBsdLicenseFile::CODE,
                crate::lint::rules::CopyrightRefersToCompressedLicense::CODE,
                crate::lint::rules::CopyrightRefersToIncorrectDirectory::CODE,
                crate::lint::rules::CopyrightRefersToVersionlessLicenseFile::CODE,
                crate::lint::rules::CopyrightRefersToNonexistentLicenseFile::CODE,
                crate::lint::rules::CopyrightFileCompressed::CODE,
                crate::lint::rules::CopyrightFileIsSymlink::CODE,
                crate::lint::rules::SpaceInStdShortnameInDep5Copyright::CODE,
                crate::lint::rules::FileInEtcNotMarkedAsConffile::CODE,
                crate::lint::rules::NoDebconfConfig::CODE,
                crate::lint::rules::UnknownField::CODE,
                crate::lint::rules::MissingSystemdServiceForInitDScript::CODE,
                crate::lint::rules::InitDScriptDoesNotSourceInitFunctions::CODE,
                crate::lint::rules::NonMultiArchLibDir::CODE,
                crate::lint::rules::PackageContainsVcsControlFile::CODE,
                crate::lint::rules::PackageInstallsDeprecatedPython2Path::CODE,
                crate::lint::rules::FileInUsrLibSitePython::CODE,
                crate::lint::rules::ThirdPartyPackageInPythonDir::CODE,
                crate::lint::rules::MenuFileInUsrLib::CODE,
                crate::lint::rules::BadMenuFileName::CODE,
                crate::lint::rules::SvkCommitFileInPackage::CODE,
                crate::lint::rules::VcsFieldUsesNotRecommendedUriFormat::CODE,
                crate::lint::rules::DebianAddsArchControlDir::CODE,
                crate::lint::rules::DebianAddsArchInventoryFile::CODE,
                crate::lint::rules::DebianAddsBtsControlDir::CODE,
                crate::lint::rules::DebianAddsBzrControlDir::CODE,
                crate::lint::rules::DebianAddsCvsConflictCopy::CODE,
                crate::lint::rules::DebianAddsCvsControlDir::CODE,
                crate::lint::rules::DebianAddsEditorBackupFile::CODE,
                crate::lint::rules::DebianAddsGitControlDir::CODE,
                crate::lint::rules::DebianAddsHgControlDir::CODE,
                crate::lint::rules::DebianAddsHgTagsFile::CODE,
                crate::lint::rules::DebianAddsPatchFailureFile::CODE,
                crate::lint::rules::DebianAddsSvkCommitFile::CODE,
                crate::lint::rules::DebianAddsSvnCommitFile::CODE,
                crate::lint::rules::DebianAddsSvnConflictFile::CODE,
                crate::lint::rules::DebianAddsSvnControlDir::CODE,
                crate::lint::rules::DebianAddsQuiltControlDir::CODE,
                crate::lint::rules::NoManualPage::CODE,
                crate::lint::rules::UncompressedManualPage::CODE,
                crate::lint::rules::WrongCompressionInManualPage::CODE,
                crate::lint::rules::CountryInManual::CODE,
                crate::lint::rules::BuildPathInManual::CODE,
                crate::lint::rules::BrokenLinkToUndocumented::CODE,
                crate::lint::rules::MaintainerScriptIgnoresErrors::CODE,
                crate::lint::rules::ReadInMaintainerScript::CODE,
                crate::lint::rules::RecursivePrivilegeChange::CODE,
                crate::lint::rules::CommandWithPathInMaintainerScript::CODE,
                crate::lint::rules::MaintainerScriptRemovesDeviceFiles::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnAdduser::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnUpdateInetd::CODE,
                crate::lint::rules::MaintainerScriptCallsGconftool::CODE,
                crate::lint::rules::MaintainerScriptCallsStartStopDaemon::CODE,
                crate::lint::rules::VirtualPackageDependsWithoutRealPackageDepends::CODE,
                crate::lint::rules::BuildDependsOnBuildEssential::CODE,
                crate::lint::rules::BuildDependsOn1Revision::CODE,
                crate::lint::rules::DependsExclusivelyOnMakedev::CODE,
                crate::lint::rules::BuildDependsOnObsoletePackage::CODE,
                crate::lint::rules::BuildDependsOnMetapackage::CODE,
                crate::lint::rules::DebhelperBaseCompatLevelIsMissing::CODE,
                crate::lint::rules::TypoInDebhelperOverrideTarget::CODE,
                crate::lint::rules::MaintainerScriptLacksDebhelperToken::CODE,
                crate::lint::rules::DebianRulesCallsPwd::CODE,
                crate::lint::rules::DebianRulesCallsDebhelperInOddOrder::CODE,
                crate::lint::rules::BinaryArchRulesButPkgIsArchIndep::CODE,
                crate::lint::rules::PackageContainsBtsControlDir::CODE,
                crate::lint::rules::UsesDpkgDatabaseDirectly::CODE,
                crate::lint::rules::ObsoleteUrlInPackaging::CODE,
                crate::lint::rules::MaintainerScriptCallsInstallSgmlcatalog::CODE,
                crate::lint::rules::MaintainerScriptChangesNetbase::CODE,
                crate::lint::rules::MaintscriptCallsLdconfig::CODE,
                crate::lint::rules::MaintainerScriptCallsChownImproperly::CODE,
                crate::lint::rules::MaintainerScriptModifiesInetdConf::CODE,
                crate::lint::rules::MaintainerScriptShouldNotUseDpkgMaintscriptHelper::CODE,
                crate::lint::rules::InstallInfoUsedInMaintainerScript::CODE,
                crate::lint::rules::MaintainerScriptHidesInitFailure::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnGconf2::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnUcf::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnXmlCore::CODE,
                crate::lint::rules::MaintainerScriptNeedsDependsOnXfontsUtils::CODE,
                crate::lint::rules::MaintainerScriptCallsInitScriptDirectly::CODE,
                crate::lint::rules::MaintainerScriptShouldNotParseEtcPasswdOrGroup::CODE,
                crate::lint::rules::InvalidArchStringInSourceRelation::CODE,
                crate::lint::rules::DependsOnMetapackage::CODE,
                crate::lint::rules::DependsOnPackagingDev::CODE,
                crate::lint::rules::DependsOnPythonMinimal::CODE,
                crate::lint::rules::NeedlesslyDependsOnAwk::CODE,
                crate::lint::rules::PackageDependsOnMultipleLibstdcVersions::CODE,
                crate::lint::rules::BuildDependsOnEssentialPackageWithoutUsingVersion::CODE,
                crate::lint::rules::PackageContainsReadmeForOtherPlatformOrDistro::CODE,
                crate::lint::rules::CompressedDocumentation::CODE,
                crate::lint::rules::FileDirectlyInUsrShareDoc::CODE,
                crate::lint::rules::MacosDsStoreFileInPackage::CODE,
                crate::lint::rules::MacosResourceForkFileInPackage::CODE,
                crate::lint::rules::PackageInstallsAppimage::CODE,
                crate::lint::rules::DebugSuffixNotDbg::CODE,
                crate::lint::rules::PackageContainsLindaOverride::CODE,
                crate::lint::rules::DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends::CODE,
                crate::lint::rules::DebhelperToolsFromAutotoolsDevAreDeprecated::CODE,
                crate::lint::rules::DebianRulesCallsNproc::CODE,
                crate::lint::rules::OverrideDhFixpermsDoesNotCallDhFixperms::CODE,
                crate::lint::rules::BraceExpansionInDebhelperConfigFile::CODE,
                crate::lint::rules::PackageUsesDhExecButLacksBuildDepends::CODE,
                crate::lint::rules::DebianControlHasDbgsymPackage::CODE,
                crate::lint::rules::BuildPrerequisiteInInstallableSection::CODE,
                crate::lint::rules::RedundantBugsField::CODE,
                crate::lint::rules::BadHomepage::CODE,
                crate::lint::rules::DescriptionContainsHomepage::CODE,
                crate::lint::rules::DescriptionSynopsisIsDuplicated::CODE,
                crate::lint::rules::FieldTooLong::CODE,
                crate::lint::rules::MissingSeparatorBetweenItems::CODE,
                crate::lint::rules::InitDScriptDoesNotImplementRequiredOption::CODE,
                crate::lint::rules::InitDScriptStopsInSRunlevel::CODE,
                crate::lint::rules::InitDScriptMissingLsbSection::CODE,
                crate::lint::rules::SystemdServiceFileRefersToObsoleteTarget::CODE,
                crate::lint::rules::MissingSystemdServiceForInitDRcSScript::CODE,
                crate::lint::rules::SystemdServiceFileWrapsInitScript::CODE,
                crate::lint::rules::UnknownFieldInTemplates::CODE,
                crate::lint::rules::PossibleDebconfNoteAbuse::CODE,
                crate::lint::rules::MalformedQuestionInTemplates::CODE,
                crate::lint::rules::UnknownDebconfPriority::CODE,
                crate::lint::rules::UsingImperativeFormInTemplates::CODE,
                crate::lint::rules::LoadsObsoleteConfmodule::CODE,
                crate::lint::rules::MakingAssumptionsAboutInterfacesInTemplates::CODE,
                crate::lint::rules::MalformedPromptInTemplates::CODE,
                crate::lint::rules::MalformedTitleInTemplates::CODE,
                crate::lint::rules::MismatchTranslatedChoices::CODE,
                crate::lint::rules::MissingDebconfDependencyForPreinst::CODE,
                crate::lint::rules::NoDebconfTemplates::CODE,
                crate::lint::rules::MenuCommandNotInPackage::CODE,
                crate::lint::rules::CommandInMenuFileAndDesktopFile::CODE,
                crate::lint::rules::MenuItemCreatesNewSection::CODE,
                crate::lint::rules::DesktopCommandNotInPackage::CODE,
                crate::lint::rules::DesktopEntryContainsUnknownKey::CODE,
                crate::lint::rules::MenuItemUsesAppsSection::CODE,
                crate::lint::rules::MenuItemUsesWindowmanagersSection::CODE,
                crate::lint::rules::NonWmInWindowmanagerMenuSection::CODE,
                crate::lint::rules::DesktopEntryFileHasCrs::CODE,
                crate::lint::rules::DesktopEntryInvalidCategory::CODE,
                crate::lint::rules::DesktopEntryUsesReservedCategory::CODE,
                crate::lint::rules::PkgNotInPackageTest::CODE,
                crate::lint::rules::ConflictsWithDependency::CODE,
                crate::lint::rules::MailTransportAgentDependencyDoesNotSpecifyDefaultMta::CODE,
                crate::lint::rules::BuildConflictsWithBuildDependency::CODE,
                crate::lint::rules::PackageDependsOnMultipleLibpngVersions::CODE,
                crate::lint::rules::PackageDependsOnMultipleTclVersions::CODE,
                crate::lint::rules::PackageDependsOnMultipleTkVersions::CODE,
                crate::lint::rules::PackageRelationWithPerlModules::CODE,
                crate::lint::rules::BadProvidedPackageName::CODE,
                crate::lint::rules::BuildDependsOnObsoleteBootstrap::CODE,
                crate::lint::rules::DependsOnObsoleteBootstrap::CODE,
                crate::lint::rules::GroffMessage::CODE,
                crate::lint::rules::EmptyManualPage::CODE,
                crate::lint::rules::UndocumentedManualPage::CODE,
                crate::lint::rules::BadSoLinkWithinManualPage::CODE,
                crate::lint::rules::BadWhatisEntry::CODE,
                crate::lint::rules::ManualPageFromTemplate::CODE,
                crate::lint::rules::ManualPageWithGenericName::CODE,
                crate::lint::rules::NoEnglishManualPage::CODE,
                crate::lint::rules::PodConversionMessage::CODE,
                crate::lint::rules::UselessWhatisEntry::CODE,
                crate::lint::rules::CopyrightFileLacksPointerToPerlLicense::CODE,
                crate::lint::rules::NoDebianCopyrightInSource::CODE,
                crate::lint::rules::CopyrightContainsDhMakeTodoBoilerplate::CODE,
                crate::lint::rules::CopyrightNotUsingCommonLicenseForGpl::CODE,
                crate::lint::rules::CopyrightFileContainsFullGplLicense::CODE,
                crate::lint::rules::CopyrightNotUsingCommonLicenseForApache2::CODE,
                crate::lint::rules::CopyrightNotUsingCommonLicenseForLgpl::CODE,
                crate::lint::rules::NamedCopyrightForSingleInstallable::CODE,
                crate::lint::rules::InitDScriptHasConflictingStartStop::CODE,
                crate::lint::rules::InitDScriptHasBadStopRunlevel::CODE,
                crate::lint::rules::InitDScriptShouldAlwaysStartService::CODE,
                crate::lint::rules::NonVirtualFacilityInInitdScript::CODE,
                crate::lint::rules::InitDScriptDependsOnAllVirtualFacility::CODE,
                crate::lint::rules::InitDScriptPossibleMissingStop::CODE,
                crate::lint::rules::InitDScriptStartsInStopRunlevel::CODE,
                crate::lint::rules::SystemdServiceAliasWithoutExtension::CODE,
                crate::lint::rules::SystemdServiceFileRefersToObsoleteBindto::CODE,
                crate::lint::rules::SystemdServiceFileUsesNobodyOrNogroup::CODE,
                crate::lint::rules::PostrmRemovesAlternative::CODE,
                crate::lint::rules::ScriptUsesBinEnv::CODE,
                crate::lint::rules::MaintainerScriptShouldNotUsePiupartsVariable::CODE,
                crate::lint::rules::SystemdAlternatives::CODE,
                crate::lint::rules::MaintainerScriptHasInvalidUpdateInetdOptions::CODE,
                crate::lint::rules::UdevadmCalledWithoutGuard::CODE,
                crate::lint::rules::MaintainerScriptChangesLdSoConf::CODE,
                crate::lint::rules::MaintainerScriptUpdatesFontconfigCacheImproperly::CODE,
                crate::lint::rules::EpochChangedButUpstreamVersionDidNotGoBackwards::CODE,
                crate::lint::rules::EpochChangeWithoutComment::CODE,
                crate::lint::rules::LatestDebianChangelogEntryReusesExistingVersion::CODE,
                crate::lint::rules::OddHistoricalDebianChangelogVersion::CODE,
                crate::lint::rules::DebianChangelogHasWrongDayOfWeek::CODE,
                crate::lint::rules::DebianChangelogLineTooLong::CODE,
                crate::lint::rules::InitialUploadClosesNoBugs::CODE,
                crate::lint::rules::NoNmuInChangelog::CODE,
                crate::lint::rules::SourceNmuHasIncorrectVersionNumber::CODE,
                crate::lint::rules::NoQaInChangelog::CODE,
            ]
        );
        let intent = LintIntent {
            subject: Subject::SourceTree(temp_dir),
            selected,
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
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
