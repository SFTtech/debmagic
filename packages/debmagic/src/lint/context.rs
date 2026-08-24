use std::cell::OnceCell;

use super::debian_control::DebianControl;
use super::diagnostic::Diagnostic;
use super::rule::{Code, Rule, Tag};
use super::subject::Subject;

pub(crate) struct LintContext<'a> {
    subject: &'a Subject,
    code: Code,
    tag: Tag,
    diagnostics: Vec<Diagnostic>,
    debian_control: OnceCell<Option<DebianControl>>,
}

impl<'a> LintContext<'a> {
    pub fn new(subject: &'a Subject, rule: &dyn Rule) -> Self {
        Self {
            subject,
            code: rule.code(),
            tag: rule.tag(),
            diagnostics: Vec::new(),
            debian_control: OnceCell::new(),
        }
    }

    pub fn subject(&self) -> &Subject {
        self.subject
    }

    /// Structured `debian/control` for a Source tree. `None` if this Subject has
    /// no such file to inspect, or if the file has a Deb822 syntax error.
    pub fn debian_control(&self) -> Option<&DebianControl> {
        self.debian_control
            .get_or_init(|| match self.subject {
                Subject::SourceTree(root) => DebianControl::load(&root.join("debian/control")),
                Subject::BinaryPackage(_) | Subject::SourcePackage(_) => None,
            })
            .as_ref()
    }

    /// Report a finding. Code and Tag come from the running Rule; Severity is
    /// set on the Diagnostic (and may be remapped later from config).
    pub fn diagnostic(&mut self, mut diagnostic: Diagnostic) {
        diagnostic.code = self.code;
        diagnostic.tag = self.tag;
        self.diagnostics.push(diagnostic);
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
