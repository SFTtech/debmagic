use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag(pub &'static str);

impl Tag {
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Code([u8; 6]);

impl Code {
    const fn uppercase_byte(byte: u8) -> u8 {
        if byte.is_ascii_lowercase() {
            byte - b'a' + b'A'
        } else {
            byte
        }
    }

    pub const fn from_bytes(bytes: [u8; 6]) -> Self {
        assert!(bytes[0].is_ascii_alphabetic());
        assert!(bytes[1].is_ascii_alphabetic());
        assert!(bytes[2].is_ascii_digit());
        assert!(bytes[3].is_ascii_digit());
        assert!(bytes[4].is_ascii_digit());
        assert!(bytes[5].is_ascii_digit());
        let mut normalized = [0u8; 6];
        let mut index = 0;
        while index < 6 {
            normalized[index] = Self::uppercase_byte(bytes[index]);
            index += 1;
        }
        Self(normalized)
    }

    pub const fn from_static(s: &'static str) -> Self {
        let bytes = s.as_bytes();
        assert!(bytes.len() == 6);
        assert!(bytes[0].is_ascii_alphabetic());
        assert!(bytes[1].is_ascii_alphabetic());
        assert!(bytes[2].is_ascii_digit());
        assert!(bytes[3].is_ascii_digit());
        assert!(bytes[4].is_ascii_digit());
        assert!(bytes[5].is_ascii_digit());
        Self::from_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]])
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 6 {
            bail!("invalid code '{s}': expected two letters and four digits");
        }
        if !bytes[0].is_ascii_alphabetic() || !bytes[1].is_ascii_alphabetic() {
            bail!("invalid code '{s}': expected two letters and four digits");
        }
        if !bytes[2..].iter().all(u8::is_ascii_digit) {
            bail!("invalid code '{s}': expected two letters and four digits");
        }
        let mut normalized = [0u8; 6];
        for (index, byte) in bytes.iter().enumerate() {
            normalized[index] = byte.to_ascii_uppercase();
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("code bytes are ASCII")
    }

    pub fn prefix(&self) -> &str {
        &self.as_str()[..2]
    }

    pub fn matches_selector_prefix(&self, prefix: &str) -> bool {
        self.as_str().starts_with(&prefix.to_ascii_uppercase())
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Pedantic,
}

impl FromStr for Severity {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            "info" => Ok(Self::Info),
            "pedantic" => Ok(Self::Pedantic),
            _ => bail!("unknown severity '{s}'"),
        }
    }
}

pub use super::subject::SubjectKind;

pub trait RuleMeta {
    const CODE: Code;
    const TAG: Tag;
    const DEFAULT_SELECTED: bool;
    const EXPERIMENTAL: bool;
    const APPLICABILITY: &'static [SubjectKind];
    #[allow(dead_code)]
    const DOCUMENTATION: &'static str;
}

/// Object-safe accessors. Associated constants on [`RuleMeta`] are not dyn-compatible.
pub trait RuleAccess: Send + Sync {
    fn code(&self) -> Code;
    fn tag(&self) -> Tag;
    fn default_selected(&self) -> bool;
    fn experimental(&self) -> bool;
    fn applicability(&self) -> &'static [SubjectKind];
    #[allow(dead_code)]
    fn documentation(&self) -> &'static str;
}

impl<T: RuleMeta + Send + Sync> RuleAccess for T {
    fn code(&self) -> Code {
        T::CODE
    }
    fn tag(&self) -> Tag {
        T::TAG
    }
    fn default_selected(&self) -> bool {
        T::DEFAULT_SELECTED
    }
    fn experimental(&self) -> bool {
        T::EXPERIMENTAL
    }
    fn applicability(&self) -> &'static [SubjectKind] {
        T::APPLICABILITY
    }
    fn documentation(&self) -> &'static str {
        T::DOCUMENTATION
    }
}

pub trait Rule: RuleAccess {
    fn run(&self, ctx: &mut super::context::LintContext<'_>);
}

#[macro_export]
macro_rules! declare_rule {
    (
        $(#[doc = $doc:literal])+
        $name:ident,
        code = $code:literal,
        tag = $tag:literal,
        default_selected = $default_selected:literal,
        experimental = $experimental:literal,
        applicability = [$($kind:ident),* $(,)?],
    ) => {
        impl $crate::lint::rule::RuleMeta for $name {
            const CODE: $crate::lint::rule::Code = $crate::lint::rule::Code::from_static($code);
            const TAG: $crate::lint::rule::Tag = $crate::lint::rule::Tag($tag);
            const DEFAULT_SELECTED: bool = $default_selected;
            const EXPERIMENTAL: bool = $experimental;
            const APPLICABILITY: &'static [$crate::lint::subject::SubjectKind] =
                &[$($crate::lint::subject::SubjectKind::$kind),*];
            const DOCUMENTATION: &'static str = concat!($($doc, "\n",)+);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_parses_and_displays_uppercase() -> anyhow::Result<()> {
        let code = Code::parse("dm0001")?;
        assert_eq!(code, Code::from_static("DM0001"));
        assert_eq!(code.to_string(), "DM0001");
        Ok(())
    }

    #[test]
    fn code_rejects_invalid_shapes() {
        assert!(Code::parse("DM001").is_err());
        assert!(Code::parse("DM00001").is_err());
        assert!(Code::parse("1M0001").is_err());
    }

    #[test]
    fn severity_parses_from_str() -> anyhow::Result<()> {
        assert_eq!("warning".parse::<Severity>()?, Severity::Warning);
        Ok(())
    }

    #[test]
    fn declare_rule_captures_multiline_documentation() {
        use crate::lint::rules::DebmagicDummyTrigger;
        let docs = DebmagicDummyTrigger::DOCUMENTATION;
        assert!(docs.contains("debian/debmagic-dummy-lint"));
        assert!(docs.contains("Selection"));
        assert!(docs.contains('\n'));
    }
}
