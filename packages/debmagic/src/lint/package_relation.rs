//! Package relation fields (Policy 7.1) as lintian's `fields/package-relations`
//! Check reads them.
//!
//! The parser is as lenient as lintian's `_split_dep`: a field is split on
//! commas and then on `|`, and each alternative is picked apart with the same
//! patterns, so malformed text never makes the field unavailable. Whatever the
//! patterns do not recognise stays on the alternative as [`Alternative::rest`]
//! (lintian's `bad-relation`). Substitution variables such as
//! `${misc:Depends}` are ordinary package names here, as in lintian.

use std::sync::LazyLock;

use regex::Regex;

static PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([^<\s\[\(]+)\s*").expect("valid regex"));
static VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\(\s*(<<|<=|>=|>>|[=<>])\s*([^\s(]+)\s*\)\s*").expect("valid regex")
});
static ARCHITECTURES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\[([^\]]+)\]\s*").expect("valid regex"));
static PROFILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*<([^>]+)>\s*").expect("valid regex"));
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));
static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\|\s*").expect("valid regex"));

/// A parsed relation field: dependencies separated by commas, each a list of
/// alternatives separated by `|`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PackageRelation {
    dependencies: Vec<Vec<Alternative>>,
}

/// `(op version)` of an alternative, `<<`, `<=`, `=`, `>=`, or `>>` as written
/// (the obsolete `<` and `>` are kept too).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VersionConstraint {
    pub(crate) operator: String,
    pub(crate) version: String,
}

/// One alternative of a dependency.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Alternative {
    text: String,
    package: String,
    architecture_qualifier: String,
    version: Option<VersionConstraint>,
    architectures: Vec<String>,
    negated_architectures: usize,
    profiles: Vec<Vec<String>>,
    rest: String,
}

impl PackageRelation {
    /// Parse an unfolded field value.
    pub(crate) fn parse(value: &str) -> Self {
        Self {
            dependencies: perl_split(&COMMA, value)
                .into_iter()
                .map(|dependency| {
                    perl_split(&PIPE, dependency)
                        .into_iter()
                        .map(Alternative::parse)
                        .collect()
                })
                .collect(),
        }
    }

    #[allow(dead_code)]
    /// Dependencies in field order; an empty entry (`a, , b`) has no
    /// alternatives.
    pub(crate) fn dependencies(&self) -> &[Vec<Alternative>] {
        &self.dependencies
    }

    /// Every alternative of every dependency, in field order.
    pub(crate) fn alternatives(&self) -> impl Iterator<Item = &Alternative> {
        self.dependencies.iter().flatten()
    }
}

// The seam exposes every part of an alternative; follow-on Tags (see
// TODO-lint.md) read the ones no Rule reads yet.
#[allow(dead_code)]
impl Alternative {
    fn parse(text: &str) -> Self {
        let mut alternative = Self {
            text: text.to_string(),
            ..Self::default()
        };
        let mut rest = text.to_string();
        if let Some(captures) = PACKAGE.captures(&rest) {
            let name = captures[1].to_string();
            let end = captures.get(0).map_or(0, |whole| whole.end());
            rest.replace_range(..end, "");
            match name.split_once(':') {
                Some((package, qualifier)) => {
                    alternative.package = package.to_string();
                    alternative.architecture_qualifier = qualifier.to_string();
                }
                None => alternative.package = name,
            }
        }
        if !rest.is_empty() {
            if let Some(captures) = VERSION.captures(&rest) {
                alternative.version = Some(VersionConstraint {
                    operator: captures[1].to_string(),
                    version: captures[2].to_string(),
                });
                rest = VERSION.replace(&rest, "").into_owned();
            }
            if !rest.is_empty()
                && let Some(captures) = ARCHITECTURES.captures(&rest)
            {
                for architecture in captures[1].split_whitespace() {
                    match architecture.strip_prefix('!') {
                        Some(name) => {
                            alternative.negated_architectures += 1;
                            alternative.architectures.push(name.to_string());
                        }
                        None => alternative.architectures.push(architecture.to_string()),
                    }
                }
                rest = ARCHITECTURES.replace(&rest, "").into_owned();
            }
            while !rest.is_empty()
                && let Some(captures) = PROFILE.captures(&rest)
            {
                alternative
                    .profiles
                    .push(captures[1].split_whitespace().map(str::to_string).collect());
                rest = PROFILE.replace(&rest, "").into_owned();
            }
        }
        alternative.rest = rest;
        alternative
    }

    /// The alternative as it stands in the field, trimmed of its separators.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Package name without the `:arch` qualifier.
    pub(crate) fn package(&self) -> &str {
        &self.package
    }

    /// The `:any`, `:native`, or `:<arch>` qualifier; empty when none.
    pub(crate) fn architecture_qualifier(&self) -> &str {
        &self.architecture_qualifier
    }

    pub(crate) fn version(&self) -> Option<&VersionConstraint> {
        self.version.as_ref()
    }

    /// Architecture names of the `[...]` restriction, without `!`.
    pub(crate) fn architectures(&self) -> &[String] {
        &self.architectures
    }

    /// How many architectures of the restriction were negated.
    pub(crate) fn negated_architectures(&self) -> usize {
        self.negated_architectures
    }

    /// Build profile groups of the `<...>` restrictions.
    pub(crate) fn profiles(&self) -> &[Vec<String>] {
        &self.profiles
    }

    /// Text left over after every recognised part is removed.
    pub(crate) fn rest(&self) -> &str {
        &self.rest
    }
}

/// Perl's `split`: a leading empty field stays, trailing empty fields go, and
/// an empty string has no fields.
fn perl_split<'a>(separator: &Regex, value: &'a str) -> Vec<&'a str> {
    let mut fields: Vec<&str> = separator.split(value).collect();
    while fields.last() == Some(&"") {
        fields.pop();
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(relation: &PackageRelation) -> Vec<Vec<&str>> {
        relation
            .dependencies()
            .iter()
            .map(|alternatives| alternatives.iter().map(Alternative::text).collect())
            .collect()
    }

    #[test]
    fn splits_on_commas_then_bars() {
        let relation = PackageRelation::parse("a, b | c (>= 1) ,d");
        assert_eq!(
            texts(&relation),
            vec![vec!["a"], vec!["b", "c (>= 1)"], vec!["d"]]
        );
    }

    #[test]
    fn empty_and_trailing_entries_follow_perl_split() {
        assert!(PackageRelation::parse("").dependencies().is_empty());
        assert_eq!(texts(&PackageRelation::parse("a,")), vec![vec!["a"]]);
        assert_eq!(
            texts(&PackageRelation::parse("a, , b")),
            vec![vec!["a"], vec![], vec!["b"]]
        );
        assert_eq!(
            texts(&PackageRelation::parse(", a")),
            vec![vec![], vec!["a"]]
        );
    }

    #[test]
    fn picks_apart_every_part() {
        let relation =
            PackageRelation::parse("libfoo:any (>= 1.2-1) [amd64 !i386] <!nocheck> <cross>");
        let alternative = relation.alternatives().next().expect("one alternative");
        assert_eq!(alternative.package(), "libfoo");
        assert_eq!(alternative.architecture_qualifier(), "any");
        assert_eq!(
            alternative.version(),
            Some(&VersionConstraint {
                operator: ">=".to_string(),
                version: "1.2-1".to_string()
            })
        );
        assert_eq!(alternative.architectures(), ["amd64", "i386"]);
        assert_eq!(alternative.negated_architectures(), 1);
        assert_eq!(
            alternative.profiles(),
            [vec!["!nocheck".to_string()], vec!["cross".to_string()]]
        );
        assert_eq!(alternative.rest(), "");
    }

    #[test]
    fn version_may_touch_the_name() {
        let relation = PackageRelation::parse("relations(<< 3)");
        let alternative = relation.alternatives().next().expect("one alternative");
        assert_eq!(alternative.package(), "relations");
        assert_eq!(alternative.version().expect("version").operator, "<<");
    }

    #[test]
    fn obsolete_operators_are_kept() {
        let relation = PackageRelation::parse("a (< 1), b (>2)");
        let operators: Vec<_> = relation
            .alternatives()
            .map(|alternative| alternative.version().expect("version").operator.clone())
            .collect();
        assert_eq!(operators, ["<", ">"]);
    }

    #[test]
    fn unrecognised_text_stays_as_rest() {
        let relation = PackageRelation::parse("a b, c (1.0), d (>= 1) junk");
        let rests: Vec<_> = relation.alternatives().map(Alternative::rest).collect();
        assert_eq!(rests, ["b", "(1.0)", "junk"]);
    }

    #[test]
    fn substitution_variables_are_names() {
        let relation = PackageRelation::parse("${misc:Depends}, ${shlibs:Depends}");
        let names: Vec<_> = relation.alternatives().map(Alternative::package).collect();
        assert_eq!(names, ["${misc", "${shlibs"]);
    }
}
