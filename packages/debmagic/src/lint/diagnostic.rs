use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use super::rule::{Code, Severity, Tag};
use super::run::override_pointer_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Lintian's package class on a Diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessableType {
    Source,
    Binary,
    Udeb,
    Changes,
    Buildinfo,
}

impl ProcessableType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Binary => "binary",
            Self::Udeb => "udeb",
            Self::Changes => "changes",
            Self::Buildinfo => "buildinfo",
        }
    }
}

impl fmt::Display for ProcessableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Cheap-to-clone handle; payload lives in [`DiagnosticInner`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Diagnostic {
    inner: Box<DiagnosticInner>,
}

/// The override that suppressed a Diagnostic: the line of its Lintian override
/// file, as lintian's `Hint::override` (ADR 0057).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overridden {
    /// The Lintian override file in its File tree.
    pub path: PathBuf,
    /// 1-based line of the override in that file.
    pub line: u32,
    /// The `#` comment above the override, joined with spaces; may be empty.
    pub justification: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticInner {
    pub message: String,
    pub location: Location,
    pub severity: Severity,
    pub code: Code,
    pub tag: Tag,
    pub package: String,
    pub processable_type: Option<ProcessableType>,
    pub help: Option<String>,
    /// Set by the override pass when a line of a Lintian override file matched.
    /// The Diagnostic stays in its Rule's order; it is not a Severity.
    pub overridden: Option<Overridden>,
}

impl Deref for Diagnostic {
    type Target = DiagnosticInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Diagnostic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Diagnostic {
    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            inner: Box::new(DiagnosticInner {
                message: message.into(),
                location: Location {
                    path: PathBuf::new(),
                    line: None,
                    column: None,
                },
                severity,
                // Replaced by the running Rule's context.
                code: Code::from_static("XX0000"),
                tag: Tag(""),
                package: String::new(),
                processable_type: None,
                help: None,
                overridden: None,
            }),
        }
    }

    pub fn with_location(mut self, location: Location) -> Self {
        self.inner.location = location;
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.inner.help = Some(help.into());
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.inner.severity = severity;
        self
    }

    pub fn with_code(mut self, code: Code) -> Self {
        self.inner.code = code;
        self
    }

    pub fn with_tag(mut self, tag: Tag) -> Self {
        self.inner.tag = tag;
        self
    }

    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.inner.package = package.into();
        self
    }

    pub fn with_processable_type(mut self, processable_type: ProcessableType) -> Self {
        self.inner.processable_type = Some(processable_type);
        self
    }

    /// Whether a line of a Lintian override file suppressed this Diagnostic.
    pub fn is_overridden(&self) -> bool {
        self.inner.overridden.is_some()
    }

    /// The override that suppressed this Diagnostic, if any.
    pub fn overridden(&self) -> Option<&Overridden> {
        self.inner.overridden.as_ref()
    }

    /// The text an override pattern is matched against, as lintian's
    /// `Hint::context`: the message, then `[pointer]` when the Location has a
    /// path. The pointer is the path, with `:line` when there is a line above
    /// zero (lintian prints no column). Empty pieces are left out.
    pub fn override_context(&self) -> String {
        let mut pieces = Vec::with_capacity(2);
        if !self.inner.message.is_empty() {
            pieces.push(self.inner.message.clone());
        }
        if !self.inner.location.path.as_os_str().is_empty() {
            let mut pointer = override_pointer_path(&self.inner.location.path);
            if let Some(line) = self.inner.location.line.filter(|&line| line > 0) {
                pointer.push_str(&format!(":{line}"));
            }
            pieces.push(format!("[{pointer}]"));
        }
        pieces.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warn_builder_sets_help_and_location() {
        let diagnostic = Diagnostic::warn("msg")
            .with_help("try this")
            .with_location(Location {
                path: PathBuf::from("debian/control"),
                line: Some(1),
                column: None,
            });
        assert_eq!(diagnostic.message, "msg");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.help.as_deref(), Some("try this"));
        assert_eq!(diagnostic.location.line, Some(1));
    }

    #[test]
    fn override_context_is_message_then_pointer() {
        let with_line = Diagnostic::warn("extra").with_location(Location {
            path: PathBuf::from("debian/control"),
            line: Some(5),
            column: None,
        });
        assert_eq!(with_line.override_context(), "extra [debian/control:5]");

        let without_line = Diagnostic::warn("extra").with_location(Location {
            path: PathBuf::from("debian/control"),
            line: None,
            column: None,
        });
        assert_eq!(without_line.override_context(), "extra [debian/control]");

        let no_message = Diagnostic::warn("").with_location(Location {
            path: PathBuf::from("usr/bin/x"),
            line: Some(1),
            column: Some(3),
        });
        assert_eq!(no_message.override_context(), "[usr/bin/x:1]");

        let no_location = Diagnostic::warn("only");
        assert_eq!(no_location.override_context(), "only");
        assert_eq!(Diagnostic::warn("").override_context(), "");
    }

    #[test]
    fn diagnostics_start_not_overridden() {
        let diagnostic = Diagnostic::warn("msg");
        assert!(!diagnostic.is_overridden());
        assert!(diagnostic.overridden().is_none());
    }

    #[test]
    fn processable_type_displays_lintian_label() {
        assert_eq!(ProcessableType::Source.to_string(), "source");
        assert_eq!(ProcessableType::Binary.to_string(), "binary");
        assert_eq!(ProcessableType::Udeb.to_string(), "udeb");
        assert_eq!(ProcessableType::Changes.to_string(), "changes");
        assert_eq!(ProcessableType::Buildinfo.to_string(), "buildinfo");
    }
}
