use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use super::rule::{Code, Severity, Tag};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Cheap-to-clone handle; payload lives in [`DiagnosticInner`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Diagnostic {
    inner: Box<DiagnosticInner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticInner {
    pub message: String,
    pub location: Location,
    pub severity: Severity,
    pub code: Code,
    pub tag: Tag,
    pub help: Option<String>,
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
                // Replaced by LintContext from the running Rule.
                code: Code::from_static("XX0000"),
                tag: Tag(""),
                help: None,
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
}
