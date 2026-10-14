use anyhow::Context;

/// The model of a package's `debian/control`.
#[derive(Debug, Clone, Default)]
pub struct Control {
    /// The source paragraph's `Rules-Requires-Root` keywords, `None` if absent.
    pub rules_requires_root: Option<Vec<String>>,
}

impl Control {
    pub fn parse(content: &str) -> anyhow::Result<Self> {
        let deb822 = content
            .parse::<deb822_lossless::Deb822>()
            .context("invalid deb822")?;
        let source = deb822.paragraphs().next().context("no source paragraph")?;
        Ok(Self {
            rules_requires_root: source
                .get("Rules-Requires-Root")
                .map(|value| value.split_whitespace().map(str::to_string).collect()),
        })
    }

    /// Whether `dpkg-buildpackage` demands a gain-root command (`fakeroot`)
    /// at startup, whatever targets it runs: unless `Rules-Requires-Root`
    /// contains `no`. An absent field counts as needing one, since dpkg
    /// before 1.22.13 defaults it to `binary-targets`.
    pub fn needs_gain_root(&self) -> bool {
        self.rules_requires_root
            .as_ref()
            .is_none_or(|keywords| !keywords.iter().any(|keyword| keyword == "no"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("Source: foo\nRules-Requires-Root: no\n", false ; "no")]
    #[test_case("Source: foo\nRules-Requires-Root: binary-targets\n", true ; "binary targets")]
    #[test_case("Source: foo\nRules-Requires-Root: dpkg/target-subcommand\n", true ; "implementation keyword")]
    #[test_case("Source: foo\n\nPackage: foo\nRules-Requires-Root: no\n", true ; "absent from source paragraph")]
    #[test_case("Source: foo\nBuild-Depends: a,\n# comment\n b\nRules-Requires-Root: no\n", false ; "comment in continuation")]
    fn test_needs_gain_root(control: &str, expected: bool) {
        assert_eq!(Control::parse(control).unwrap().needs_gain_root(), expected);
    }
}
