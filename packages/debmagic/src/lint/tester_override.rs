//! Rule-level harness for `OverrideRule`s.
//!
//! A case is one processable's Lintian override file, the Tags of the Rules
//! that ran on it, and the Diagnostics those Rules made. The harness runs the
//! override pass on them, then the Rule on what the pass left unmatched, as
//! `run.rs` does. Pass cases must make no Diagnostic; fail cases must make one.

use std::path::PathBuf;

use super::diagnostic::{Diagnostic, ProcessableType};
use super::lintian_overrides::LintianOverrides;
use super::override_pass::{self, OverrideContext, ProcessableFacts};
use super::rule::{OverrideRule, Tag};
use super::run::format_diagnostic;

/// One processable: a Source tree (architecture `source`) unless
/// [`OverrideCase::binary`] says otherwise.
#[derive(Clone, Debug)]
pub struct OverrideCase {
    overrides: String,
    architecture: String,
    processable_type: ProcessableType,
    ran: Vec<&'static str>,
    diagnostics: Vec<Diagnostic>,
}

impl OverrideCase {
    /// A Source tree whose `debian/source/lintian-overrides` is `overrides`.
    pub fn new(overrides: impl Into<String>) -> Self {
        Self {
            overrides: overrides.into(),
            architecture: "source".to_string(),
            processable_type: ProcessableType::Source,
            ran: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// A Binary package of `architecture`, whose override file is
    /// `usr/share/lintian/overrides/example`.
    pub fn binary(mut self, architecture: &str) -> Self {
        self.architecture = architecture.to_string();
        self.processable_type = ProcessableType::Binary;
        self
    }

    /// Tags of Rules that ran on the processable.
    pub fn ran(mut self, tags: &[&'static str]) -> Self {
        self.ran.extend_from_slice(tags);
        self
    }

    /// A Diagnostic another Rule made on the processable.
    pub fn diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }
}

/// Rule-level test harness for an [`OverrideRule`]; see the module docs.
pub struct OverrideTester {
    rule: Box<dyn OverrideRule>,
    expect_pass: Vec<OverrideCase>,
    expect_fail: Vec<OverrideCase>,
}

impl OverrideTester {
    pub fn new<R, P, F, C>(rule: R, expect_pass: P, expect_fail: F) -> Self
    where
        R: OverrideRule + 'static,
        P: IntoIterator<Item = C>,
        F: IntoIterator<Item = C>,
        C: Into<OverrideCase>,
    {
        Self {
            rule: Box::new(rule),
            expect_pass: expect_pass.into_iter().map(Into::into).collect(),
            expect_fail: expect_fail.into_iter().map(Into::into).collect(),
        }
    }

    pub fn test(&mut self) {
        let mut problems = Vec::new();
        for case in &self.expect_pass {
            let output = self.run(case);
            if !output.is_empty() {
                problems.push(format!(
                    "expected to pass, but {:?} reported:\n{}",
                    case.overrides,
                    output.join("\n")
                ));
            }
        }
        for case in &self.expect_fail {
            if self.run(case).is_empty() {
                problems.push(format!("expected to fail, but {:?} passed", case.overrides));
            }
        }
        assert!(
            problems.is_empty(),
            "Some tests failed for Rule {} [{}]:\n{}",
            self.rule.code(),
            self.rule.tag(),
            problems.join("\n")
        );
    }

    /// The lines the Rule reported for `case`, empty when it reported nothing.
    fn run(&self, case: &OverrideCase) -> Vec<String> {
        let (path, package) = match case.processable_type {
            ProcessableType::Binary | ProcessableType::Udeb => {
                ("usr/share/lintian/overrides/example", "example")
            }
            _ => ("debian/source/lintian-overrides", "example"),
        };
        let file = LintianOverrides::parse(
            PathBuf::from(path),
            &case.overrides,
            package,
            case.processable_type,
        );
        let ran: Vec<Tag> = case.ran.iter().copied().map(Tag).collect();
        let mut diagnostics = case.diagnostics.clone();
        let facts = ProcessableFacts {
            architecture: &case.architecture,
            ran_tags: &ran,
        };
        let outcome = override_pass::apply(&file, &facts, &mut diagnostics);
        let mut ctx = OverrideContext::bind(
            self.rule.as_ref(),
            outcome,
            file.path(),
            package,
            case.processable_type,
        );
        self.rule.run(&mut ctx);
        ctx.finish().iter().map(format_diagnostic).collect()
    }
}
