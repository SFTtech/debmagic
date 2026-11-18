use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use super::rule::{Code, Severity, Tag};

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
}

impl ProcessableType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Binary => "binary",
            Self::Udeb => "udeb",
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
    fn processable_type_displays_lintian_label() {
        assert_eq!(ProcessableType::Source.to_string(), "source");
        assert_eq!(ProcessableType::Binary.to_string(), "binary");
        assert_eq!(ProcessableType::Udeb.to_string(), "udeb");
    }
}
