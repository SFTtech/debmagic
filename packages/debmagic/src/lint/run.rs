use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use rayon::prelude::*;

use super::context::{BinaryPackageContext, SourceContext, SourceTreeData};
use super::diagnostic::Diagnostic;
use super::group::Group;
use super::group_context::{BuildinfoContext, ChangesContext, GroupContext};
use super::intent::LintIntent;
use super::registry;
use super::rule::{Code, RuleAccess, Severity, Tag};
use super::subject::{LintTarget, Subject};
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
    let mut result = pool.install(|| match &intent.target {
        LintTarget::SourceTree(path) => run_source_tree_lint(intent, path, started),
        LintTarget::Artifacts(subjects) => run_artifact_lint(intent, subjects, started),
    })?;
    result.elapsed = started.elapsed();
    result.threads = threads;
    Ok(result)
}

/// Lint the Groups the artifacts make up (ADR 0054, 0055). Each artifact of a
/// Group is a Subject of its own kind, opened only when a selected Rule
/// applies to it; the Group as a whole is the Subject of the Group Rules.
fn run_artifact_lint(
    intent: &LintIntent,
    subjects: &[Subject],
    started: Instant,
) -> anyhow::Result<LintResult> {
    let groups = Group::assemble(subjects)?;
    let selected: HashSet<Code> = intent.selected.iter().copied().collect();
    let mut stages = Vec::new();
    for group in &groups {
        if let Some(changes) = &group.changes
            && any_selected(registry::changes_rules(), &selected)
        {
            stages.push(run_stage(
                intent,
                registry::changes_rules(),
                started,
                |rule| {
                    let mut ctx = ChangesContext::bind(changes, rule);
                    rule.run(&mut ctx);
                    Ok(ctx.finish())
                },
            )?);
        }
        if let Some(source) = &group.source {
            if any_selected(registry::source_rules(), &selected) {
                let data = source
                    .package()
                    .map_err(|message| anyhow::anyhow!("{message}"))?;
                stages.push(run_stage(
                    intent,
                    registry::source_rules(),
                    started,
                    |rule| {
                        let mut ctx = SourceContext::bind_package(data, rule);
                        rule.run(&mut ctx);
                        ctx.finish()
                    },
                )?);
            }
            if any_selected(registry::group_rules(), &selected) {
                stages.push(run_stage(
                    intent,
                    registry::group_rules(),
                    started,
                    |rule| {
                        let mut ctx = GroupContext::bind(group, rule);
                        rule.run(&mut ctx);
                        ctx.finish()
                    },
                )?);
            }
        }
        if let Some(buildinfo) = &group.buildinfo
            && any_selected(registry::buildinfo_rules(), &selected)
        {
            stages.push(run_stage(
                intent,
                registry::buildinfo_rules(),
                started,
                |rule| {
                    let mut ctx = BuildinfoContext::bind(buildinfo, rule);
                    rule.run(&mut ctx);
                    Ok(ctx.finish())
                },
            )?);
        }
        if any_selected(registry::binary_package_rules(), &selected) {
            for binary in &group.binaries {
                stages.push(run_stage(
                    intent,
                    registry::binary_package_rules(),
                    started,
                    |rule| {
                        let mut ctx = BinaryPackageContext::bind(binary, rule);
                        rule.run(&mut ctx);
                        ctx.finish()
                    },
                )?);
            }
        }
    }
    finish_run(intent, stages, started)
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
    let stage = run_stage(intent, registry::source_rules(), started, |rule| {
        let mut ctx = SourceContext::bind_tree(&data, rule);
        rule.run(&mut ctx);
        ctx.finish()
    })?;
    finish_run(intent, vec![stage], started)
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

/// What running the selected Rules of one kind on one Subject produced.
struct Stage {
    diagnostics: Vec<Diagnostic>,
    rules: Vec<RuleTiming>,
    bringup: Duration,
    shared: Duration,
}

fn any_selected<R: RuleAccess + ?Sized>(rules: &[&R], selected: &HashSet<Code>) -> bool {
    rules.iter().any(|rule| selected.contains(&rule.code()))
}

/// Run every selected Rule of `rules` on the current rayon pool. Rules are
/// independent, so they run in parallel; diagnostics are collected in Rule
/// order, so output does not depend on the thread count. If several Rules
/// fail, the first in Rule order is reported.
///
/// `started` is the beginning of the lint run. Time up to the moment Rules
/// launch is bringup. Shared initialization that happens inside a Rule is
/// subtracted from that Rule and reported once.
fn run_stage<R: RuleAccess + ?Sized>(
    intent: &LintIntent,
    rules: &[&R],
    started: Instant,
    run_rule: impl Fn(&R) -> anyhow::Result<Vec<Diagnostic>> + Sync,
) -> anyhow::Result<Stage> {
    let selected = intent.selected.iter().copied().collect::<HashSet<_>>();
    let execution: Vec<_> = rules
        .iter()
        .copied()
        .filter(|rule| selected.contains(&rule.code()))
        .collect();

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
    Ok(Stage {
        diagnostics,
        rules: rule_timings,
        bringup,
        shared,
    })
}

/// Put the stages of a run together: diagnostics in stage order, one timing
/// per Rule (the time it took on every Subject it ran on), and the Rules that
/// ran at least once. If Selection is non-empty and no Rule ran anywhere, the
/// selected Rules do not apply to what was named (ADR 0018).
fn finish_run(
    intent: &LintIntent,
    stages: Vec<Stage>,
    started: Instant,
) -> anyhow::Result<LintResult> {
    let mut diagnostics = Vec::new();
    let mut rule_timings: Vec<RuleTiming> = Vec::new();
    let mut shared = Duration::ZERO;
    let bringup = stages
        .first()
        .map_or_else(|| started.elapsed(), |stage| stage.bringup);
    for stage in stages {
        diagnostics.extend(stage.diagnostics);
        shared += stage.shared;
        for timing in stage.rules {
            match rule_timings
                .iter_mut()
                .find(|known| known.code == timing.code)
            {
                Some(known) => known.elapsed += timing.elapsed,
                None => rule_timings.push(timing),
            }
        }
    }
    if !intent.selected.is_empty() && rule_timings.is_empty() {
        bail!("none of the selected Rules apply to this Subject");
    }

    let outcome = outcome_for(&diagnostics, &intent.fail_on);
    Ok(LintResult {
        diagnostics,
        outcome,
        rules: rule_timings.len(),
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
        let mut pointer = pointer_path(&diagnostic.location.path);
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

/// A Location path as lintian prints a pointer: as is when it is UTF-8,
/// otherwise every byte read as Latin-1, so the bytes stay visible.
pub(crate) fn pointer_path(path: &Path) -> String {
    match path.to_str() {
        Some(text) => text.to_string(),
        None => path
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .map(|&byte| char::from(byte))
            .collect(),
    }
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
    use crate::lint::context::{BinaryPackageData, SourcePackageData};
    use crate::lint::diagnostic::{Location, ProcessableType};
    use crate::lint::intent::{LintIntentInput, OverrideMode, resolve_lint_intent};
    use crate::lint::rule::{RuleMeta, Severity};
    use crate::lint::rules::{DebmagicDummyTrigger, RequiredField};
    use crate::lint::selection::resolve_selected_codes;

    fn base_intent(subject: PathBuf) -> LintIntent {
        resolve_lint_intent(LintIntentInput {
            fallback_dir: subject.clone(),
            subjects: vec![subject],
            config_file: None,
            select: vec![],
            ignore: vec![],
            fail_on: vec![],
            threads: None,
            timings: false,
            overrides: OverrideMode::default(),
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
    fn format_diagnostic_prints_non_utf8_pointer_as_latin1() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let diagnostic = Diagnostic::error("").with_location(Location {
            path: PathBuf::from(OsStr::from_bytes(b"usr/share/foo/bokm\xe5l")),
            line: None,
            column: None,
        });
        assert!(format_diagnostic(&diagnostic).ends_with(" [usr/share/foo/bokm\u{e5}l]"));
        let utf8 = Diagnostic::error("").with_location(Location {
            path: PathBuf::from("usr/bin/\u{9ce5}"),
            line: None,
            column: None,
        });
        assert!(format_diagnostic(&utf8).ends_with(" [usr/bin/\u{9ce5}]"));
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
            target: LintTarget::SourceTree(temp_dir),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::SourceTree(temp_dir),
            selected: vec![DebmagicDummyTrigger::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: Some(1),
            timings: true,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::SourceTree(temp_dir),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: Some(1),
            timings: false,
            overrides: OverrideMode::Hide,
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
                target: LintTarget::SourceTree(temp_dir.clone()),
                selected: selected.clone(),
                fail_on: HashSet::from([Severity::Error]),
                severity_remaps: HashMap::new(),
                threads,
                timings: false,
                overrides: OverrideMode::Hide,
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
            target: LintTarget::Artifacts(vec![Subject::BinaryPackage(std::path::absolute(
                &path,
            )?)]),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
        let mut elf = std::fs::read(std::env::current_exe()?)?;
        // LN0005 skips an ELF executable that has the OCaml magic as a
        // standalone printable run, and whether this test binary has one
        // depends on how the linker lays out its literals. Break every
        // occurrence (same length) so the skip can never apply. The marker is
        // built from bytes so this test adds no literal of its own.
        let marker = *b"Caml1999X";
        let mut broken = marker;
        broken[8] = b'Y';
        let mut at = 0;
        while let Some(found) = elf[at..].windows(marker.len()).position(|w| w == marker) {
            elf[at + found..at + found + marker.len()].copy_from_slice(&broken);
            at += found + marker.len();
        }
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
            target: LintTarget::Artifacts(vec![Subject::BinaryPackage(std::path::absolute(
                &path,
            )?)]),
            selected: vec![crate::lint::rules::UnstrippedBinaryOrObject::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::Artifacts(vec![Subject::BinaryPackage(std::path::absolute(
                &path,
            )?)]),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::Artifacts(vec![Subject::BinaryPackage(std::path::absolute(
                &path,
            )?)]),
            selected: vec![crate::lint::rules::SyntaxErrorInDebianChangelog::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::Artifacts(vec![Subject::SourcePackage(std::path::absolute(&dsc)?)]),
            selected: vec![crate::lint::rules::SourceContainsPrebuiltWindowsBinary::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
            target: LintTarget::Artifacts(vec![Subject::SourcePackage(std::path::absolute(&dsc)?)]),
            selected: vec![RequiredField::CODE],
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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
             Section: devel\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.4\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n",
        )?;
        fs::create_dir_all(temp_dir.join("debian/source"))?;
        fs::write(temp_dir.join("debian/source/format"), "3.0 (native)\n")?;
        fs::write(
            temp_dir.join("debian/copyright"),
            "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
             \n\
             Files: *\n\
             Copyright: 2024 Example\n\
             License: Expat\n\
             \x20Permission is hereby granted.\n",
        )?;
        let selected = resolve_selected_codes(&["LN".to_string()], &[])?;
        // A Code prefix selects every Rule with that prefix, default-selected
        // or not, sorted by Code (ADR 0014).
        let mut expected: Vec<Code> = crate::lint::registry::all_rules()
            .iter()
            .map(|rule| rule.code())
            .filter(|code| code.prefix() == "LN")
            .collect();
        expected.sort();
        assert_eq!(selected, expected);
        assert!(selected.contains(&crate::lint::rules::RequiredField::CODE));
        let intent = LintIntent {
            target: LintTarget::SourceTree(temp_dir),
            selected,
            fail_on: HashSet::from([Severity::Error]),
            severity_remaps: HashMap::new(),
            threads: None,
            timings: false,
            overrides: OverrideMode::Hide,
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

    fn tempfile_dir(prefix: &str) -> anyhow::Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("cleaning {}", dir.display()))?;
        }
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(dir)
    }
}
