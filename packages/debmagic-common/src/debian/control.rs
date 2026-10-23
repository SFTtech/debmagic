use anyhow::Context;

/// The source paragraph's build relationship fields, `Build-Depends` first:
/// a [`BuildScope::Clean`] needs only that one.
const BUILD_DEPENDS_FIELDS: [&str; 3] =
    ["Build-Depends", "Build-Depends-Arch", "Build-Depends-Indep"];
const BUILD_CONFLICTS_FIELDS: [&str; 3] = [
    "Build-Conflicts",
    "Build-Conflicts-Arch",
    "Build-Conflicts-Indep",
];

/// Which build relationship fields a build has to satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildScope {
    /// `debian/rules clean` (and so a source build): `Build-Depends` and
    /// `Build-Conflicts` only.
    Clean,
    /// A full binary build: also the `-Arch` and `-Indep` fields.
    Full,
}

/// The relations a build has to satisfy, one comma-separated list per field,
/// with arch qualifiers and build profiles still in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildRelations {
    pub depends: Vec<String>,
    pub conflicts: Vec<String>,
}

/// The model of a package's `debian/control`.
#[derive(Debug, Clone, Default)]
pub struct Control {
    /// The source paragraph's `Rules-Requires-Root` keywords, `None` if absent.
    pub rules_requires_root: Option<Vec<String>>,
    /// The source paragraph's build relationship fields in the order of
    /// [`BUILD_DEPENDS_FIELDS`] and [`BUILD_CONFLICTS_FIELDS`], normalized
    /// to one line without empty entries; `None` if absent or empty.
    pub build_depends: [Option<String>; 3],
    pub build_conflicts: [Option<String>; 3],
}

impl Control {
    pub fn parse(content: &str) -> anyhow::Result<Self> {
        let deb822 = content
            .parse::<deb822_lossless::Deb822>()
            .context("invalid deb822")?;
        let source = deb822.paragraphs().next().context("no source paragraph")?;
        let relations = |fields: [&str; 3]| {
            fields.map(|field| {
                source
                    .get(field)
                    .and_then(|value| normalize_relations(&value))
            })
        };
        Ok(Self {
            rules_requires_root: source
                .get("Rules-Requires-Root")
                .map(|value| value.split_whitespace().map(str::to_string).collect()),
            build_depends: relations(BUILD_DEPENDS_FIELDS),
            build_conflicts: relations(BUILD_CONFLICTS_FIELDS),
        })
    }

    /// The build relations `scope` has to satisfy.
    pub fn build_relations(&self, scope: BuildScope) -> BuildRelations {
        let fields = match scope {
            BuildScope::Clean => 1,
            BuildScope::Full => 3,
        };
        let collect = |relations: &[Option<String>; 3]| {
            relations[..fields].iter().flatten().cloned().collect()
        };
        BuildRelations {
            depends: collect(&self.build_depends),
            conflicts: collect(&self.build_conflicts),
        }
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

/// A relations field on one line, without the empty entries a trailing
/// comma leaves (old apt rejects them); `None` if nothing remains.
fn normalize_relations(value: &str) -> Option<String> {
    let relations: Vec<String> = value
        .split(',')
        .map(|relation| relation.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|relation| !relation.is_empty())
        .collect();
    (!relations.is_empty()).then(|| relations.join(", "))
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

    const BUILD_RELATIONS: &str = "Source: foo\n\
        Build-Depends: debhelper-compat (= 13),\n\
        # comment\n\
        \x20libfoo-dev [amd64] <!nocheck>,\n\
        Build-Depends-Indep: python3-sphinx\n\
        Build-Conflicts: libbar-dev\n\
        Build-Conflicts-Arch:\n\
        \n\
        Package: foo\n\
        Build-Depends: ignored\n";

    #[test_case(BuildScope::Clean, &["debhelper-compat (= 13), libfoo-dev [amd64] <!nocheck>"] ; "clean")]
    #[test_case(BuildScope::Full, &["debhelper-compat (= 13), libfoo-dev [amd64] <!nocheck>", "python3-sphinx"] ; "full")]
    fn test_build_relations(scope: BuildScope, depends: &[&str]) {
        let relations = Control::parse(BUILD_RELATIONS)
            .unwrap()
            .build_relations(scope);
        assert_eq!(
            relations,
            BuildRelations {
                depends: depends
                    .iter()
                    .map(|relation| relation.to_string())
                    .collect(),
                conflicts: vec!["libbar-dev".to_string()],
            }
        );
    }
}
