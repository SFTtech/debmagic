/// The source package format from `debian/source/format`.
///
/// dpkg-source defaults to `1.0` when the file is absent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    #[default]
    V1,
    Quilt,
    Native,
}

impl SourceFormat {
    /// Parse the content of a `debian/source/format` file; anything
    /// but the known 3.0 formats is treated as `1.0`, like dpkg-source.
    pub fn parse(content: &str) -> Self {
        match content.trim() {
            "3.0 (native)" => Self::Native,
            "3.0 (quilt)" => Self::Quilt,
            _ => Self::V1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "1.0",
            Self::Quilt => "3.0 (quilt)",
            Self::Native => "3.0 (native)",
        }
    }

    /// A native package has no `orig` tarball at all.
    pub fn is_native(self) -> bool {
        self == Self::Native
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("3.0 (native)\n", SourceFormat::Native; "native")]
    #[test_case("3.0 (quilt)", SourceFormat::Quilt; "quilt")]
    #[test_case("1.0\n", SourceFormat::V1; "v1")]
    #[test_case("garbage", SourceFormat::V1; "unknown is v1")]
    #[test_case("", SourceFormat::V1; "empty is v1")]
    fn test_parse(content: &str, expected: SourceFormat) {
        assert_eq!(SourceFormat::parse(content), expected);
    }

    #[test]
    fn test_as_str_roundtrip() {
        for format in [SourceFormat::V1, SourceFormat::Quilt, SourceFormat::Native] {
            assert_eq!(SourceFormat::parse(format.as_str()), format);
        }
    }

    #[test]
    fn test_is_native() {
        assert!(SourceFormat::Native.is_native());
        assert!(!SourceFormat::Quilt.is_native());
        assert!(!SourceFormat::V1.is_native());
    }
}
