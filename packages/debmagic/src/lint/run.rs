use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use rayon::prelude::*;

use super::context::{
    BinaryPackageContext, BinaryPackageData, SourceContext, SourceTreeData,
    processable_type_for_binary,
};
use super::diagnostic::Diagnostic;
use super::diagnostic::ProcessableType;
use super::group::{DscData, Group};
use super::group_context::{BuildinfoContext, ChangesContext, GroupContext};
use super::intent::{LintIntent, OverrideMode};
use super::lintian_overrides::LintianOverrides;
use super::override_pass::{self, OverrideContext, ProcessableFacts};
use super::registry;
use super::rule::{Code, GroupRule, OverrideRule, RuleAccess, Severity, Tag};
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
    /// Overrides of Show-Always Tags, which cannot suppress anything (ADR 0057).
    pub ignored_overrides: usize,
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
    let threads = worker_threads(intent.threads, intent.selected.len());
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
/// After a Group's Rules ran, the override pass applies each processable's
/// Lintian override file (ADR 0057).
fn run_artifact_lint(
    intent: &LintIntent,
    subjects: &[Subject],
    started: Instant,
) -> anyhow::Result<LintResult> {
    let groups = Group::assemble(subjects)?;
    let selected: HashSet<Code> = intent.selected.iter().copied().collect();
    let mut stages = Vec::new();
    let mut ignored_overrides = 0;
    for group in &groups {
        let mut group_stages = Vec::new();
        if let Some(changes) = &group.changes
            && any_selected(registry::changes_rules(), &selected)
        {
            group_stages.push(run_stage(
                intent,
                Owner::Changes,
                registry::changes_rules(),
                started,
                |rule| {
                    let mut ctx = ChangesContext::bind(changes, rule);
                    rule.run(&mut ctx);
                    Ok(ctx.finish())
                },
            )?);
        }
        if let Some(source) = &group.source
            && any_selected(registry::source_rules(), &selected)
        {
            let data = source
                .package()
                .map_err(|message| anyhow::anyhow!("{message}"))?;
            group_stages.push(run_stage(
                intent,
                Owner::Source,
                registry::source_rules(),
                started,
                |rule| {
                    let mut ctx = SourceContext::bind_package(data, rule);
                    rule.run(&mut ctx);
                    ctx.finish()
                },
            )?);
        }
        let group_rules = applicable_group_rules(registry::group_rules(), group);
        if any_selected(&group_rules, &selected) {
            group_stages.push(run_stage(
                intent,
                Owner::Group,
                &group_rules,
                started,
                |rule| {
                    let mut ctx = GroupContext::bind(group, rule);
                    rule.run(&mut ctx);
                    ctx.finish()
                },
            )?);
        }
        if let Some(buildinfo) = &group.buildinfo
            && any_selected(registry::buildinfo_rules(), &selected)
        {
            group_stages.push(run_stage(
                intent,
                Owner::Buildinfo,
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
            for (index, binary) in group.binaries.iter().enumerate() {
                group_stages.push(run_stage(
                    intent,
                    Owner::Binary(index),
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
        let mut processables = Vec::new();
        if let Some(source) = &group.source {
            processables.push(Processable::Source(source, group.source_name()));
        }
        processables.extend(
            group
                .binaries
                .iter()
                .enumerate()
                .map(|(index, binary)| Processable::Binary(index, binary)),
        );
        ignored_overrides += apply_overrides(
            intent.overrides,
            &selected,
            &processables,
            &mut group_stages,
        )?;
        stages.extend(group_stages);
    }
    finish_run(intent, stages, ignored_overrides, started)
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
    let mut stages = vec![run_stage(
        intent,
        Owner::Source,
        registry::source_rules(),
        started,
        |rule| {
            let mut ctx = SourceContext::bind_tree(&data, rule);
            rule.run(&mut ctx);
            ctx.finish()
        },
    )?];
    let selected: HashSet<Code> = intent.selected.iter().copied().collect();
    let ignored_overrides = apply_overrides(
        intent.overrides,
        &selected,
        &[Processable::Tree(&data)],
        &mut stages,
    )?;
    finish_run(intent, stages, ignored_overrides, started)
}

/// Most worker threads a run uses unless `--threads` says otherwise. Past
/// this, wider pools add CPU (idle workers, startup) without shortening a
/// samba Subject's wall time.
const DEFAULT_MAX_THREADS: usize = 16;

/// Worker threads for a run: the requested count exactly when given (0 counts
/// as unset). Otherwise the available parallelism, capped by the number of
/// selected Rules and by `DEFAULT_MAX_THREADS`, and never below one.
fn worker_threads(requested: Option<usize>, selected_rules: usize) -> usize {
    if let Some(threads) = requested.filter(|&threads| threads > 0) {
        return threads;
    }
    let parallelism: usize = std::thread::available_parallelism()
        .map(Into::into)
        .unwrap_or(1);
    parallelism
        .min(selected_rules)
        .clamp(1, DEFAULT_MAX_THREADS)
}

struct RuleProduct {
    diagnostics: Vec<Diagnostic>,
    timing: RuleTiming,
}

/// Which part of the run a stage ran on: the processable its Diagnostics
/// belong to, before the override pass attributes them (ADR 0057).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Changes,
    Source,
    Group,
    Buildinfo,
    /// The installable at this index of the Group.
    Binary(usize),
    /// The `OverrideRule`s of one processable: their Diagnostics belong to no
    /// processable's override pass (ADR 0057).
    Override,
}

/// What running the selected Rules of one kind on one Subject produced.
struct Stage {
    owner: Owner,
    diagnostics: Vec<Diagnostic>,
    rules: Vec<RuleTiming>,
    bringup: Duration,
    shared: Duration,
}

/// A processable the override pass covers: one Source tree, the Source package
/// of a Group, or one installable of a Group (ADR 0057).
enum Processable<'g> {
    Tree(&'g SourceTreeData),
    /// The Source package of the Group, with the Group's source name.
    Source(&'g DscData, &'g str),
    Binary(usize, &'g BinaryPackageData),
}

impl Processable<'_> {
    /// The package name Diagnostics of this processable carry.
    fn package_name(&self) -> String {
        match self {
            Self::Tree(data) => data.package_name(),
            Self::Source(_, name) => (*name).to_string(),
            Self::Binary(_, binary) => binary.package_name().to_string(),
        }
    }

    /// Lintian's processable type: `source`, or `binary` / `udeb` by file name.
    fn processable_type(&self) -> ProcessableType {
        match self {
            Self::Tree(_) | Self::Source(..) => ProcessableType::Source,
            Self::Binary(_, binary) => processable_type_for_binary(binary.path()),
        }
    }

    /// Whether a Diagnostic of a stage with `owner` belongs to this processable.
    /// A Group Rule's Diagnostic belongs to the source or to the installable it
    /// names (its package and processable type, as `installable_diagnostic` sets
    /// them); two installables of one name share such Diagnostics.
    fn owns(&self, owner: Owner, diagnostic: &Diagnostic) -> bool {
        match (self, owner) {
            (Self::Tree(_), Owner::Source) | (Self::Source(..), Owner::Source) => true,
            (Self::Source(..), Owner::Group) => {
                diagnostic.processable_type == Some(ProcessableType::Source)
            }
            (Self::Binary(index, _), Owner::Binary(other)) => *index == other,
            (Self::Binary(_, binary), Owner::Group) => {
                diagnostic.processable_type == Some(processable_type_for_binary(binary.path()))
                    && diagnostic.package == binary.package_name()
            }
            _ => false,
        }
    }

    /// Whether a Rule of a stage with `owner` ran on this processable. A Group
    /// stage holds exactly the Group Rules that ran on the Group: all selected
    /// ones for a Group with a `.dsc`, and for a lone `.deb` only those that
    /// opt in with `runs_without_dsc` (ADR 0056). Each of them counts as run on
    /// the Source package and on every installable of its Group, as lintian's
    /// Checks do; a Rule that reports nothing there leaves its override unused.
    fn ran_here(&self, owner: Owner) -> bool {
        match (self, owner) {
            (Self::Tree(_) | Self::Source(..), Owner::Source | Owner::Group) => true,
            (Self::Binary(index, _), Owner::Binary(other)) => *index == other,
            (Self::Binary(..), Owner::Group) => true,
            _ => false,
        }
    }

    /// Lintian's `Processable::architecture`: `source` for a Source tree or
    /// Source package, the control `Architecture` of an installable.
    fn architecture(&self) -> &str {
        match self {
            Self::Tree(_) | Self::Source(..) => "source",
            Self::Binary(_, binary) => binary
                .installation_control()
                .fields()
                .get("Architecture")
                .unwrap_or_default()
                .trim(),
        }
    }

    /// The Lintian override file of this processable, read through a context
    /// that reports nothing. `None` when there is none, and when the File tree
    /// it is read from fails: lintian then has no override file (its
    /// `Processable::overrides` is empty) and does not fail the run, so that
    /// failure is left to the Rules that read the same File tree. This departs
    /// from ADR 0057's "runtime failure" for a File tree (see the report of the
    /// task). An extract failure of a Source package is still a runtime failure.
    fn load_overrides(&self) -> anyhow::Result<Option<LintianOverrides>> {
        match self {
            Self::Tree(data) => {
                let mut ctx = SourceContext::reading_tree(data);
                let overrides = ctx.lintian_overrides();
                let _ = ctx.finish();
                Ok(overrides)
            }
            Self::Source(dsc, _) => {
                let package = dsc
                    .package()
                    .map_err(|message| anyhow::anyhow!("{message}"))?;
                let mut ctx = SourceContext::reading_package(package);
                let overrides = ctx.lintian_overrides();
                let _ = ctx.finish();
                Ok(overrides)
            }
            Self::Binary(_, binary) => {
                let mut ctx = BinaryPackageContext::reading(binary);
                let overrides = ctx.lintian_overrides();
                let _ = ctx.finish();
                Ok(overrides)
            }
        }
    }
}

/// Applies the override pass to each processable of a Group or Source tree,
/// after all its Rules ran (ADR 0057). Sets `overridden` on the Diagnostics
/// that a line of the processable's override file suppressed, in place. The
/// override file is read only for a processable with Diagnostics or when an
/// `OverrideRule` is selected. The selected `OverrideRule`s then run on the
/// pass's outcome, in a stage of their own. Returns how many overrides of
/// Show-Always Tags were ignored.
fn apply_overrides(
    mode: OverrideMode,
    selected: &HashSet<Code>,
    processables: &[Processable<'_>],
    stages: &mut Vec<Stage>,
) -> anyhow::Result<usize> {
    drop_repeated_hints(processables, stages);
    if mode == OverrideMode::Off {
        return Ok(0);
    }
    let override_rules: Vec<&dyn OverrideRule> = registry::override_rules()
        .iter()
        .copied()
        .filter(|rule| selected.contains(&rule.code()))
        .collect();
    let mut ignored = 0;
    for processable in processables {
        let mut members = Vec::new();
        for (stage_index, stage) in stages.iter().enumerate() {
            for (index, diagnostic) in stage.diagnostics.iter().enumerate() {
                if processable.owns(stage.owner, diagnostic) {
                    members.push((stage_index, index));
                }
            }
        }
        if members.is_empty() && override_rules.is_empty() {
            continue;
        }
        let overrides = processable.load_overrides()?;
        if overrides.is_none() && !override_rules.is_empty() {
            // No override file: the selected OverrideRules still ran, and found nothing.
            stages.push(run_override_rules(
                &override_rules,
                processable,
                Path::new(""),
                &override_pass::PassOutcome::default(),
            ));
        }
        let Some(overrides) = overrides else {
            continue;
        };
        let ran_tags: Vec<Tag> = stages
            .iter()
            .flat_map(|stage| {
                stage
                    .rules
                    .iter()
                    .map(move |timing| (stage.owner, timing.tag))
            })
            .filter(|&(owner, _)| processable.ran_here(owner))
            .map(|(_, tag)| tag)
            .collect();
        let facts = ProcessableFacts {
            architecture: processable.architecture(),
            ran_tags: &ran_tags,
        };
        let mut subset: Vec<Diagnostic> = members
            .iter()
            .map(|&(stage_index, index)| stages[stage_index].diagnostics[index].clone())
            .collect();
        let outcome = override_pass::apply(&overrides, &facts, &mut subset);
        ignored += outcome.ignored_durable;
        for (&(stage_index, index), updated) in members.iter().zip(subset) {
            stages[stage_index].diagnostics[index].overridden = updated.overridden.clone();
        }
        if !override_rules.is_empty() {
            stages.push(run_override_rules(
                &override_rules,
                processable,
                overrides.path(),
                &outcome,
            ));
        }
    }
    Ok(ignored)
}

/// lintian's `Lintian::Group` issues a hint once per processable: a repeat of
/// the same tag and context is dropped (with a warning) before the overrides
/// see it, so the override pass and the counts never meet the copy. The context
/// is the text lintian matches, the note and the pointer.
fn drop_repeated_hints(processables: &[Processable<'_>], stages: &mut [Stage]) {
    let mut repeated: HashSet<(usize, usize)> = HashSet::new();
    for processable in processables {
        let mut seen: HashSet<(Code, String)> = HashSet::new();
        for (stage_index, stage) in stages.iter().enumerate() {
            for (index, diagnostic) in stage.diagnostics.iter().enumerate() {
                if processable.owns(stage.owner, diagnostic)
                    && !seen.insert((diagnostic.code, diagnostic.override_context()))
                {
                    repeated.insert((stage_index, index));
                }
            }
        }
    }
    for (stage_index, stage) in stages.iter_mut().enumerate() {
        let mut index = 0;
        stage.diagnostics.retain(|_| {
            let keep = !repeated.contains(&(stage_index, index));
            index += 1;
            keep
        });
    }
}

/// Runs the selected `OverrideRule`s on one processable's pass outcome (ADR
/// 0057). Their Diagnostics form a stage that no processable claims, so they
/// are never overridden.
fn run_override_rules(
    rules: &[&dyn OverrideRule],
    processable: &Processable<'_>,
    path: &Path,
    outcome: &override_pass::PassOutcome,
) -> Stage {
    let package = processable.package_name();
    let processable_type = processable.processable_type();
    let mut diagnostics = Vec::new();
    let mut timings = Vec::with_capacity(rules.len());
    for rule in rules {
        let start = Instant::now();
        let mut ctx =
            OverrideContext::bind(*rule, outcome.clone(), path, &package, processable_type);
        rule.run(&mut ctx);
        diagnostics.extend(ctx.finish());
        timings.push(RuleTiming {
            code: rule.code(),
            tag: rule.tag(),
            elapsed: start.elapsed(),
        });
    }
    Stage {
        owner: Owner::Override,
        diagnostics,
        rules: timings,
        bringup: Duration::ZERO,
        shared: Duration::ZERO,
    }
}

fn any_selected<R: RuleAccess + ?Sized>(rules: &[&R], selected: &HashSet<Code>) -> bool {
    rules.iter().any(|rule| selected.contains(&rule.code()))
}

/// The Group Rules that run on `group`: all of them for a Group with a `.dsc`;
/// for a Group without one (a lone `.deb`, or `.deb`s and a `.changes`), only
/// the Rules that report on an installable (ADR 0056).
fn applicable_group_rules<'r>(
    rules: &[&'r dyn GroupRule],
    group: &Group,
) -> Vec<&'r dyn GroupRule> {
    rules
        .iter()
        .copied()
        .filter(|rule| group.source.is_some() || rule.runs_without_dsc())
        .collect()
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
    owner: Owner,
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
        owner,
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
    ignored_overrides: usize,
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
        ignored_overrides,
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

/// FailOn counts the Diagnostics no override suppressed (ADR 0057).
pub(crate) fn outcome_for(diagnostics: &[Diagnostic], fail_on: &HashSet<Severity>) -> LintOutcome {
    if diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.is_overridden())
        .any(|diagnostic| fail_on.contains(&diagnostic.severity))
    {
        LintOutcome::PolicyFailure
    } else {
        LintOutcome::Success
    }
}

/// The lines `debmagic lint` prints for the Diagnostics (ADR 0057). An
/// overridden Diagnostic is printed only under `Show`, with the letter `O`, and
/// after an `N:` line holding its justification when there is one.
pub fn format_diagnostic_lines(diagnostics: &[Diagnostic], mode: OverrideMode) -> Vec<String> {
    let mut lines = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.is_overridden() {
            if mode != OverrideMode::Show {
                continue;
            }
            let justification = diagnostic
                .overridden()
                .map(|overridden| overridden.justification.as_str())
                .unwrap_or_default();
            if !justification.is_empty() {
                lines.push(format!("N: {justification}"));
            }
        }
        lines.push(format_diagnostic(diagnostic));
    }
    lines
}

/// The `N:` lines after the summary (ADR 0057): under `Hide`, what was
/// overridden and which overrides are unused; and the note on Show-Always
/// overrides that were ignored. Nothing under `Off`.
pub fn format_override_notes(result: &LintResult, mode: OverrideMode) -> Vec<String> {
    let mut lines = Vec::new();
    if mode == OverrideMode::Off {
        return lines;
    }
    if mode == OverrideMode::Hide {
        let overridden = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_overridden())
            .count();
        let unused = result
            .diagnostics
            .iter()
            .filter(|diagnostic| is_override_report(diagnostic))
            .count();
        if overridden > 0 || unused > 0 {
            let mut parts = Vec::new();
            if overridden > 0 {
                parts.push(format!(
                    "{overridden} {} overridden",
                    if overridden == 1 { "hint" } else { "hints" }
                ));
            }
            if unused > 0 {
                parts.push(format!(
                    "{unused} unused {}",
                    if unused == 1 { "override" } else { "overrides" }
                ));
            }
            lines.push(format!("N: {}", parts.join("; ")));
        }
    }
    if result.ignored_overrides > 0 {
        lines.push("N: Some overrides were ignored.".to_string());
    }
    lines
}

/// Whether a Diagnostic is `unused-override` or `mismatched-override` (ADR 0057).
fn is_override_report(diagnostic: &Diagnostic) -> bool {
    matches!(diagnostic.tag.0, "unused-override" | "mismatched-override")
}

pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let letter = if diagnostic.is_overridden() {
        'O'
    } else {
        diagnostic.severity.letter()
    };
    let mut output = format!("{letter}:");
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

/// A Location path as lintian prints it in a hint: lintian keeps every item
/// name as undecoded bytes, reads each byte as a Latin-1 character, and prints
/// a non-printable as `?` (`EWI::_quote_print`). So `公共` shows as `å?¬å?±`,
/// and a name in valid UTF-8 is not shown as UTF-8. Printable means not an
/// ASCII control (0x00 to 0x1f, 0x7f) and not a C1 control (0x80 to 0x9f).
pub(crate) fn pointer_path(path: &Path) -> String {
    path.as_os_str()
        .as_encoded_bytes()
        .iter()
        .map(|&byte| match byte {
            0x00..=0x1f | 0x7f..=0x9f => '?',
            _ => char::from(byte),
        })
        .collect()
}

/// The Location path as the override context spells it: UTF-8 as is, other
/// bytes read as Latin-1. An override pattern is matched against this text.
pub(crate) fn override_pointer_path(path: &Path) -> String {
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
    // Overridden Diagnostics are not counted (ADR 0057).
    let reported: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.is_overridden())
        .collect();
    if reported.is_empty() {
        return "All checks passed!".to_string();
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    let mut pedantic = 0usize;
    for diagnostic in reported {
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
    use crate::lint::tester_group::GroupCase;

    /// A Group Rule that reports on the source package only.
    struct OnSourceRule;

    crate::declare_rule! {
        /// Reports on the source package.
        OnSourceRule,
        code = "DM9997",
        tag = "group-on-source-only",
        default_selected = false,
        experimental = true,
    }

    impl GroupRule for OnSourceRule {
        fn run(&self, _ctx: &mut GroupContext<'_>) {}
    }

    /// A Group Rule that reports on an installable, so it runs without a `.dsc`.
    struct OnInstallableRule;

    crate::declare_rule! {
        /// Reports on an installable.
        OnInstallableRule,
        code = "DM9996",
        tag = "group-on-installable-only",
        default_selected = false,
        experimental = true,
    }

    impl GroupRule for OnInstallableRule {
        fn run(&self, _ctx: &mut GroupContext<'_>) {}

        fn runs_without_dsc(&self) -> bool {
            true
        }
    }

    #[test]
    fn group_without_dsc_runs_only_installable_group_rules() {
        let rules: [&dyn GroupRule; 2] = [&OnSourceRule, &OnInstallableRule];
        let with_dsc = GroupCase::new("Source: example\nVersion: 1.0\n").group();
        let lone_deb = GroupCase::without_dsc().installable("example", "").group();
        let codes = |group: &Group| -> Vec<Code> {
            applicable_group_rules(&rules, group)
                .iter()
                .map(|rule| rule.code())
                .collect()
        };
        assert_eq!(
            codes(&with_dsc),
            [OnSourceRule::CODE, OnInstallableRule::CODE]
        );
        assert_eq!(codes(&lone_deb), [OnInstallableRule::CODE]);
    }

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
    fn format_diagnostic_prints_pointer_bytes_as_lintian_does() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let diagnostic = Diagnostic::error("").with_location(Location {
            path: PathBuf::from(OsStr::from_bytes(b"usr/share/foo/bokm\xe5l")),
            line: None,
            column: None,
        });
        assert!(format_diagnostic(&diagnostic).ends_with(" [usr/share/foo/bokm\u{e5}l]"));
        // UTF-8 `公共` is the bytes E5 85 AC E5 85 B1: C1 bytes print as `?`.
        let utf8 = Diagnostic::error("").with_location(Location {
            path: PathBuf::from("usr/bin/\u{516c}\u{5171}"),
            line: None,
            column: None,
        });
        assert!(format_diagnostic(&utf8).ends_with(" [usr/bin/\u{e5}?\u{ac}\u{e5}?\u{b1}]"));
        assert_eq!(
            override_pointer_path(Path::new("usr/bin/\u{516c}")),
            "usr/bin/\u{516c}"
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
    fn worker_threads_honours_request_and_caps_the_default() {
        // An explicit count is exact, even above the Rule count or the default cap.
        assert_eq!(worker_threads(Some(8), 1), 8);
        assert_eq!(worker_threads(Some(40), 500), 40);
        assert_eq!(worker_threads(Some(1), 500), 1);
        // Unset or zero: the machine, capped by the Rules and DEFAULT_MAX_THREADS.
        let parallelism: usize = std::thread::available_parallelism().map_or(1, Into::into);
        assert_eq!(
            worker_threads(Some(0), 500),
            parallelism.min(DEFAULT_MAX_THREADS)
        );
        assert_eq!(worker_threads(None, 1), 1);
        assert_eq!(
            worker_threads(None, 500),
            parallelism.min(DEFAULT_MAX_THREADS)
        );
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
            assert_eq!(result.threads, worker_threads(threads, selected.len()));
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
            selected: vec![crate::lint::rules::NoDep5Copyright::CODE],
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

#[cfg(test)]
mod override_application_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    use crate::lint::diagnostic::{Location, Overridden};

    /// A Source tree whose `debian/source/lintian-overrides` is `overrides`.
    fn tree_with_overrides(overrides: &str) -> anyhow::Result<(tempfile::TempDir, SourceTreeData)> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("debian/source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("lintian-overrides"), overrides)?;
        let data = SourceTreeData::new(dir.path().to_path_buf());
        Ok((dir, data))
    }

    fn stage(owner: Owner, tag: &'static str, diagnostics: Vec<Diagnostic>) -> Stage {
        Stage {
            owner,
            diagnostics,
            rules: vec![RuleTiming {
                code: Code::from_static("LN1370"),
                tag: Tag(tag),
                elapsed: Duration::ZERO,
            }],
            bringup: Duration::ZERO,
            shared: Duration::ZERO,
        }
    }

    fn cute(message: &str) -> Diagnostic {
        Diagnostic::warn(message)
            .with_tag(Tag("cute-field"))
            .with_code(Code::from_static("LN1370"))
            .with_location(Location {
                path: PathBuf::from("debian/control"),
                line: None,
                column: None,
            })
    }

    fn flagged(message: &str, justification: &str) -> Diagnostic {
        let mut diagnostic = Diagnostic::warn(message);
        diagnostic.overridden = Some(Overridden {
            path: PathBuf::from("debian/source/lintian-overrides"),
            line: 2,
            justification: justification.to_string(),
        });
        diagnostic
    }

    /// lintian `Group.pm` drops a repeat of a Tag and context before the
    /// overrides, so the repeat is neither reported nor counted by an override.
    #[test]
    fn repeated_hints_are_dropped_before_the_override_pass() -> anyhow::Result<()> {
        let (_dir, data) = tree_with_overrides("cute-field HOMEPAGE*\n")?;
        let mut stages = vec![stage(
            Owner::Source,
            "cute-field",
            vec![
                cute("HOMEPAGE vs Homepage"),
                cute("HOMEPAGE vs Homepage"),
                cute("X-Dh-Compat vs X-DH-Compat"),
                cute("X-Dh-Compat vs X-DH-Compat"),
            ],
        )];
        let ignored = apply_overrides(
            OverrideMode::Hide,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        assert_eq!(ignored, 0);
        let flags: Vec<bool> = stages[0]
            .diagnostics
            .iter()
            .map(Diagnostic::is_overridden)
            .collect();
        assert_eq!(flags, [true, false]);
        Ok(())
    }

    #[test]
    fn source_tree_override_flags_only_matching_diagnostics() -> anyhow::Result<()> {
        let (_dir, data) = tree_with_overrides("cute-field HOMEPAGE*\n")?;
        let mut stages = vec![stage(
            Owner::Source,
            "cute-field",
            vec![
                cute("HOMEPAGE vs Homepage"),
                cute("X-Dh-Compat vs X-DH-Compat"),
            ],
        )];
        let ignored = apply_overrides(
            OverrideMode::Hide,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        assert_eq!(ignored, 0);
        let flags: Vec<bool> = stages[0]
            .diagnostics
            .iter()
            .map(Diagnostic::is_overridden)
            .collect();
        assert_eq!(flags, [true, false]);
        Ok(())
    }

    /// lintian `Group.pm` tries the patterns of a Tag with `sort_by { length }`
    /// (List::UtilsBy compares the lengths as strings), so `10` comes before `9`:
    /// of two matching patterns the longer one suppresses the Diagnostic.
    #[test]
    fn matching_patterns_are_tried_in_string_order_of_their_length() -> anyhow::Result<()> {
        let (_dir, data) = tree_with_overrides("cute-field *control]\ncute-field */control]\n")?;
        let mut stages = vec![stage(Owner::Source, "cute-field", vec![cute("x")])];
        apply_overrides(
            OverrideMode::Hide,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        let line = stages[0].diagnostics[0].overridden.as_ref().map(|o| o.line);
        assert_eq!(line, Some(2));
        Ok(())
    }

    #[test]
    fn off_mode_never_reads_or_flags() -> anyhow::Result<()> {
        let (_dir, data) = tree_with_overrides("cute-field\n")?;
        let mut stages = vec![stage(Owner::Source, "cute-field", vec![cute("x")])];
        apply_overrides(
            OverrideMode::Off,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        assert!(!stages[0].diagnostics[0].is_overridden());
        Ok(())
    }

    #[test]
    fn processable_without_diagnostics_does_not_read_its_file() -> anyhow::Result<()> {
        // An unreadable override file is never read when nothing was reported.
        let (dir, data) = tree_with_overrides("cute-field\n")?;
        fs::write(
            dir.path().join("debian/source/lintian-overrides"),
            [0xff, 0xfe],
        )?;
        let mut stages = vec![stage(Owner::Source, "cute-field", vec![])];
        let ignored = apply_overrides(
            OverrideMode::Hide,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        assert_eq!(ignored, 0);
        Ok(())
    }

    #[test]
    fn durable_override_is_counted_and_suppresses_nothing() -> anyhow::Result<()> {
        let (_dir, data) = tree_with_overrides("malformed-override\ncute-field\n")?;
        let mut stages = vec![stage(Owner::Source, "cute-field", vec![cute("x")])];
        // The durable Tag's Rule ran, so its override is seen and ignored.
        stages[0].rules.push(RuleTiming {
            code: Code::from_static("LN1234"),
            tag: Tag("malformed-override"),
            elapsed: Duration::ZERO,
        });
        let ignored = apply_overrides(
            OverrideMode::Hide,
            &HashSet::new(),
            &[Processable::Tree(&data)],
            &mut stages,
        )?;
        assert_eq!(ignored, 1);
        assert!(stages[0].diagnostics[0].is_overridden());
        Ok(())
    }

    /// A `.deb` of `example` whose lintian override file is `overrides`.
    fn binary_with_overrides(
        overrides: &str,
    ) -> anyhow::Result<(tempfile::TempDir, BinaryPackageData)> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("example_1.0_amd64.deb");
        crate::lint::binary_package_reader::write_test_deb_with_files(
            &path,
            &[(
                "control",
                "Package: example\nVersion: 1.0\nArchitecture: amd64\nDescription: example\n extra\n"
                    .as_bytes(),
            )],
            &[(
                "usr/share/lintian/overrides/example",
                overrides.as_bytes(),
            )],
        )?;
        let data = BinaryPackageData::new(path)?;
        Ok((dir, data))
    }

    /// An installable Diagnostic of a Group Rule, as `installable_diagnostic` sets it.
    fn installable_hint(tag: &'static str, message: &str, path: &str) -> Diagnostic {
        Diagnostic::warn(message)
            .with_tag(Tag(tag))
            .with_code(Code::from_static("LN1503"))
            .with_package("example")
            .with_processable_type(ProcessableType::Binary)
            .with_location(Location {
                path: PathBuf::from(path),
                line: None,
                column: None,
            })
    }

    const LIBRARY: &str = "lacks-unversioned-link-to-shared-library";

    /// lintian's `Group::process` judges an override of an installable-reporting
    /// Group Rule's Tag on each installable, the Group having a `.dsc` (ADR 0056):
    /// the Rule ran on the binary, so an override whose pattern matches is used.
    #[test]
    fn group_rule_override_on_installable_is_used_when_it_matches() -> anyhow::Result<()> {
        let (_dir, data) = binary_with_overrides(&format!("{LIBRARY} example: *\n"))?;
        let mut stages = vec![stage(
            Owner::Group,
            LIBRARY,
            vec![installable_hint(
                LIBRARY,
                "example: usr/lib/libexample.so",
                "usr/lib/libexample.so",
            )],
        )];
        apply_overrides(
            OverrideMode::Hide,
            &HashSet::from([Code::from_static("LN1503")]),
            &[Processable::Binary(0, &data)],
            &mut stages,
        )?;
        assert!(stages[0].diagnostics[0].is_overridden());
        Ok(())
    }

    /// The same Group Rule ran on the binary and reported nothing matching the
    /// pattern, so lintian's `unused-override` applies (ADR 0057, LN1779).
    #[test]
    fn group_rule_override_on_installable_is_unused_when_nothing_matches() -> anyhow::Result<()> {
        let (_dir, data) = binary_with_overrides(&format!("{LIBRARY} usr/lib/*/libother.so\n"))?;
        let mut stages = vec![stage(Owner::Group, LIBRARY, vec![])];
        apply_overrides(
            OverrideMode::Hide,
            &HashSet::from([Code::from_static("LN1779")]),
            &[Processable::Binary(0, &data)],
            &mut stages,
        )?;
        let unused: Vec<String> = stages
            .iter()
            .flat_map(|stage| stage.diagnostics.iter())
            .filter(|diagnostic| diagnostic.tag.0 == "unused-override")
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        assert_eq!(unused, [format!("{LIBRARY} usr/lib/*/libother.so")]);
        Ok(())
    }

    /// An override for a Tag whose Rule did not run on the Group is neither applied
    /// nor reported: its Rule is not known to have judged this installable.
    #[test]
    fn override_for_a_tag_whose_group_rule_did_not_run_takes_no_part() -> anyhow::Result<()> {
        let (_dir, data) = binary_with_overrides(&format!(
            "file-references-package-build-path example: *\n{LIBRARY} example: *\n"
        ))?;
        let mut stages = vec![stage(
            Owner::Group,
            LIBRARY,
            vec![installable_hint(
                LIBRARY,
                "example: usr/lib/libexample.so",
                "usr/lib/libexample.so",
            )],
        )];
        apply_overrides(
            OverrideMode::Hide,
            &HashSet::from([Code::from_static("LN1779")]),
            &[Processable::Binary(0, &data)],
            &mut stages,
        )?;
        let unused = stages
            .iter()
            .flat_map(|stage| stage.diagnostics.iter())
            .filter(|diagnostic| diagnostic.tag.0 == "unused-override")
            .count();
        assert_eq!(unused, 0);
        assert!(stages[0].diagnostics[0].is_overridden());
        Ok(())
    }

    #[test]
    fn hide_prints_nothing_for_overridden_and_show_prints_o_after_n() {
        let diagnostics = vec![
            flagged("HOMEPAGE vs Homepage", "kept on purpose"),
            cute("other"),
        ];

        let hide = format_diagnostic_lines(&diagnostics, OverrideMode::Hide);
        assert_eq!(hide.len(), 1, "{hide:?}");
        assert!(hide[0].starts_with("W:"), "{hide:?}");

        let show = format_diagnostic_lines(&diagnostics, OverrideMode::Show);
        assert_eq!(show.len(), 3, "{show:?}");
        assert_eq!(show[0], "N: kept on purpose");
        assert!(show[1].starts_with("O:"), "{show:?}");
        assert!(show[2].starts_with("W:"), "{show:?}");
    }

    #[test]
    fn overridden_diagnostics_do_not_count_for_failon_or_summary() {
        let fail_on = HashSet::from([Severity::Error]);
        let overridden = {
            let mut error = Diagnostic::error("e");
            error.overridden = Some(Overridden {
                path: PathBuf::new(),
                line: 1,
                justification: String::new(),
            });
            error
        };
        assert_eq!(
            outcome_for(std::slice::from_ref(&overridden), &fail_on),
            LintOutcome::Success
        );
        assert_eq!(
            format_summary(std::slice::from_ref(&overridden)),
            "All checks passed!"
        );
        assert_eq!(
            outcome_for(&[overridden, Diagnostic::error("e")], &fail_on),
            LintOutcome::PolicyFailure
        );
    }

    #[test]
    fn override_notes_follow_the_mode() {
        let result = LintResult {
            diagnostics: vec![flagged("a", ""), flagged("b", ""), flagged("c", "")],
            outcome: LintOutcome::Success,
            rules: 1,
            ignored_overrides: 0,
            elapsed: Duration::ZERO,
            threads: 1,
            timings: None,
        };
        assert_eq!(
            format_override_notes(&result, OverrideMode::Hide),
            ["N: 3 hints overridden"]
        );
        assert!(format_override_notes(&result, OverrideMode::Show).is_empty());
        assert!(format_override_notes(&result, OverrideMode::Off).is_empty());

        let ignored = LintResult {
            diagnostics: vec![],
            ignored_overrides: 2,
            ..result
        };
        assert_eq!(
            format_override_notes(&ignored, OverrideMode::Show),
            ["N: Some overrides were ignored."]
        );
    }
}
