//! Package relation fields (Policy 7.1) as lintian's `fields/package-relations`
//! Check reads them.
//!
//! The parser is as lenient as lintian's `_split_dep`: a field is split on
//! commas and then on `|`, and each alternative is picked apart with the same
//! patterns, so malformed text never makes the field unavailable. Whatever the
//! patterns do not recognise stays on the alternative as [`Alternative::rest`]
//! (lintian's `bad-relation`). Substitution variables such as
//! `${misc:Depends}` are ordinary package names here, as in `_split_dep`.
//!
//! [`PackageRelation::satisfies`] is lintian's `Relation::satisfies`: it
//! reparses each alternative with the predicate grammar, so a substvar stays
//! one package name, and answers whether this relation implies the condition.

use std::cmp::Ordering;
use std::str::FromStr;
use std::sync::LazyLock;

use debmagic_common::debian::version::PackageVersion;
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
/// lintian's `Relation::Predicate` grammar. A substvar is one package name,
/// including the colon inside `${...}`.
static PREDICATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^\s*
        (
          (?: [a-zA-Z0-9][a-zA-Z0-9+.\-]* | \$\{[a-zA-Z0-9:\-]+\} )
          (?: [a-zA-Z0-9+.\-]+ | \$\{[a-zA-Z0-9:\-]+\} )*
        )
        (?: : ([a-z0-9\-]+) )?
        (?: \s* \( \s* (<<|<=|>=|>>|[=<>]) \s* ([^)]+) \s* \) )?
        (?: \s* \[ \s* ([^\]]+) \s* \] )?
        (?: \s* < \s* ([^,]+) \s* > )?
        \s*$
        ",
    )
    .expect("valid regex")
});

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

    /// AND of `self` and `other`, skipping an empty relation.
    ///
    /// lintian's `logical_and`. Comma-separated dependencies of `other` follow
    /// those of `self`.
    pub(crate) fn and(&self, other: &Self) -> Self {
        if other.dependencies.iter().all(Vec::is_empty) {
            return self.clone();
        }
        let mut dependencies = self.dependencies.clone();
        dependencies.extend(
            other
                .dependencies
                .iter()
                .filter(|dependency| !dependency.is_empty())
                .cloned(),
        );
        Self { dependencies }
    }

    /// Whether this relation implies `condition`.
    ///
    /// True when every package set that satisfies this relation also satisfies
    /// `condition` (lintian's `Relation::satisfies`; an unknown implication is
    /// false). An empty condition is implied. Architecture restrictions are
    /// honoured. A build profile on either side is not an implication.
    pub(crate) fn satisfies(&self, condition: &str) -> bool {
        implies(&formula_from(&self.dependencies), &Formula::load(condition))
    }

    /// lintian's `restriction_less` (`relation_norestriction`): every
    /// alternative the predicate grammar parses keeps its name (and a `:any`
    /// qualifier, any other qualifier is dropped), without version,
    /// architecture restriction, or build profile; an alternative it cannot
    /// parse stays as written. Empty dependencies are dropped.
    pub(crate) fn restriction_less(&self) -> Self {
        let dependencies = self
            .dependencies
            .iter()
            .filter(|dependency| !dependency.is_empty())
            .map(|dependency| {
                dependency
                    .iter()
                    .map(|alternative| {
                        let predicate = Predicate::parse(alternative.text());
                        if predicate.parsable {
                            let qualifier = if predicate.qualifier == "any" {
                                ":any"
                            } else {
                                ""
                            };
                            Alternative::parse(&format!("{}{qualifier}", predicate.name))
                        } else {
                            alternative.clone()
                        }
                    })
                    .collect()
            })
            .collect();
        Self { dependencies }
    }

    /// lintian's `to_string`: standardised spacing, substvars kept whole, and
    /// an alternative the predicate grammar cannot parse left as written.
    pub(crate) fn to_lintian_string(&self) -> String {
        formula_from(&self.dependencies).display()
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

/// One alternative as lintian's `Relation::Predicate` parses it.
struct Predicate {
    literal: String,
    parsable: bool,
    name: String,
    qualifier: String,
    operator: String,
    version: String,
    /// Architecture restriction as captured, spaces included.
    architecture: String,
    /// Architectures of the restriction, `!` kept on negated names.
    architectures: Vec<String>,
    /// The one `<...>` build-profile group, when the grammar parsed one.
    profile: String,
}

impl Predicate {
    fn parse(text: &str) -> Self {
        let Some(captures) = PREDICATE.captures(text) else {
            return Self {
                literal: text.to_string(),
                parsable: false,
                name: String::new(),
                qualifier: String::new(),
                operator: String::new(),
                version: String::new(),
                architecture: String::new(),
                architectures: Vec::new(),
                profile: String::new(),
            };
        };
        let architecture = captures.get(5).map_or("", |m| m.as_str()).to_string();
        let mut operator = captures.get(3).map_or("", |m| m.as_str()).to_string();
        if operator == "<" {
            operator = "<<".to_string();
        } else if operator == ">" {
            operator = ">>".to_string();
        }
        Self {
            literal: text.to_string(),
            parsable: true,
            name: captures[1].to_string(),
            qualifier: captures.get(2).map_or("", |m| m.as_str()).to_string(),
            version: captures.get(4).map_or("", |m| m.as_str()).to_string(),
            architectures: architecture
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            architecture,
            operator,
            profile: captures.get(6).map_or("", |m| m.as_str()).to_string(),
        }
    }

    /// lintian's `to_string`. An unparsable alternative is the original text.
    fn display(&self) -> String {
        if !self.parsable {
            return self.literal.clone();
        }
        let mut text = self.name.clone();
        if !self.qualifier.is_empty() {
            text.push(':');
            text.push_str(&self.qualifier);
        }
        if !self.operator.is_empty() {
            text.push_str(&format!(" ({} {})", self.operator, self.version));
        }
        if !self.architecture.is_empty() {
            text.push_str(&format!(" [{}]", self.architecture));
        }
        if !self.profile.is_empty() {
            text.push_str(&format!(" <{}>", self.profile));
        }
        text
    }
}

/// A relation as an AND of predicates and OR-groups.
enum Formula {
    And(Vec<Formula>),
    Or(Vec<Formula>),
    Pred(Predicate),
}

impl Formula {
    fn load(condition: &str) -> Self {
        let mut clauses = Vec::new();
        for requirement in perl_split(&COMMA, condition) {
            if requirement.is_empty() {
                continue;
            }
            let alternatives = perl_split(&PIPE, requirement);
            if alternatives.is_empty() {
                continue;
            }
            clauses.push(or_clause(
                alternatives
                    .into_iter()
                    .map(|text| Self::Pred(Predicate::parse(text)))
                    .collect(),
            ));
        }
        Self::And(clauses)
    }

    fn display(&self) -> String {
        match self {
            Self::And(clauses) | Self::Or(clauses) => {
                let separator = if matches!(self, Self::And(_)) {
                    ", "
                } else {
                    " | "
                };
                clauses
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(separator)
            }
            Self::Pred(predicate) => predicate.display(),
        }
    }
}

fn formula_from(dependencies: &[Vec<Alternative>]) -> Formula {
    let mut clauses = Vec::new();
    for dependency in dependencies {
        if dependency.is_empty() {
            continue;
        }
        clauses.push(or_clause(
            dependency
                .iter()
                .map(|alternative| Formula::Pred(Predicate::parse(alternative.text())))
                .collect(),
        ));
    }
    Formula::And(clauses)
}

fn or_clause(mut predicates: Vec<Formula>) -> Formula {
    if predicates.len() == 1 {
        predicates.pop().expect("one predicate")
    } else {
        Formula::Or(predicates)
    }
}

/// `true` when `have` implies `want`. Unknown is `false`, as in
/// `Relation::satisfies`.
fn implies(have: &Formula, want: &Formula) -> bool {
    match want {
        Formula::Pred(_) => match have {
            Formula::Pred(_) => predicate_implies(have, want),
            Formula::And(clauses) => clauses.iter().any(|clause| implies(clause, want)),
            Formula::Or(clauses) => {
                !clauses.is_empty() && clauses.iter().all(|clause| implies(clause, want))
            }
        },
        Formula::And(wanted) => wanted.iter().all(|clause| implies(have, clause)),
        Formula::Or(wanted) => match have {
            Formula::Pred(_) => wanted.iter().any(|clause| implies(have, clause)),
            Formula::And(clauses) => clauses.iter().any(|clause| implies(clause, want)),
            Formula::Or(clauses) => {
                !clauses.is_empty()
                    && clauses.iter().all(|clause| {
                        wanted
                            .iter()
                            .any(|wanted_clause| implies(clause, wanted_clause))
                    })
            }
        },
    }
}

fn predicate_implies(have: &Formula, want: &Formula) -> bool {
    let (Formula::Pred(have), Formula::Pred(want)) = (have, want) else {
        return false;
    };
    if !have.parsable || !want.parsable {
        return have.display() == want.display();
    }
    if have.name != want.name || !have.profile.is_empty() || !want.profile.is_empty() {
        return false;
    }
    if !architectures_imply(&have.architectures, &want.architectures) {
        return false;
    }
    if !qualifiers_imply(&have.qualifier, &want.qualifier) {
        return false;
    }
    version_implies(&have.operator, &have.version, &want.operator, &want.version)
}

/// Architectures of `have` must be a superset of `want`. No list is every
/// architecture. The first name's `!` decides whether the list is negated,
/// and the `!` stays part of the name, as in lintian.
fn architectures_imply(have: &[String], want: &[String]) -> bool {
    if have.is_empty() {
        return true;
    }
    if want.is_empty() {
        return false;
    }
    let have_negated = have[0].starts_with('!');
    let want_negated = want[0].starts_with('!');
    if have_negated && want_negated {
        have.iter().all(|architecture| want.contains(architecture))
    } else if !have_negated && !want_negated {
        want.iter().all(|architecture| have.contains(architecture))
    } else if have_negated {
        !have.iter().any(|architecture| {
            architecture
                .get(1..)
                .is_some_and(|bare| want.iter().any(|wanted| wanted == bare))
        })
    } else {
        false
    }
}

fn qualifiers_imply(have: &str, want: &str) -> bool {
    if have.is_empty() && !want.is_empty() {
        return want == "any";
    }
    if !have.is_empty() && want.is_empty() {
        return false;
    }
    have == want
}

fn version_implies(have_op: &str, have_version: &str, want_op: &str, want_version: &str) -> bool {
    if want_op.is_empty() {
        return true;
    }
    if have_op.is_empty() {
        return false;
    }
    match want_op {
        "=" => have_op == "=" && version_matches(have_version, "=", want_version),
        "<=" => match have_op {
            "=" | "<<" | "<=" => version_matches(have_version, "<=", want_version),
            _ => false,
        },
        // A `<<` relation implies another `<<` when its version is `<=` the
        // other's (anything below 1 is below 1). `<=` and `=` must be strictly
        // below, because equality would miss `<<`.
        "<<" => match have_op {
            "<<" => version_matches(have_version, "<=", want_version),
            "<=" | "=" => version_matches(have_version, "<<", want_version),
            _ => false,
        },
        ">=" => match have_op {
            "=" | ">>" | ">=" => version_matches(have_version, ">=", want_version),
            _ => false,
        },
        ">>" => match have_op {
            ">>" => version_matches(have_version, ">=", want_version),
            ">=" | "=" => version_matches(have_version, ">>", want_version),
            _ => false,
        },
        _ => false,
    }
}

fn version_matches(left: &str, operator: &str, right: &str) -> bool {
    let Some(order) = version_ord(left, right) else {
        return false;
    };
    match operator {
        "=" => order == Ordering::Equal,
        "<=" => order != Ordering::Greater,
        ">=" => order != Ordering::Less,
        "<<" => order == Ordering::Less,
        ">>" => order == Ordering::Greater,
        _ => false,
    }
}

/// dpkg's version order. Equal strings compare equal before parsing, as
/// lintian's `versions_equal` does. Surrounding space from the predicate
/// capture is not part of the version.
fn version_ord(left: &str, right: &str) -> Option<Ordering> {
    if left == right {
        return Some(Ordering::Equal);
    }
    let left = PackageVersion::from_str(left.trim()).ok()?;
    let right = PackageVersion::from_str(right.trim()).ok()?;
    Some(left.cmp(&right))
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

    #[test]
    fn satisfies_names_versions_and_alternatives() {
        assert!(PackageRelation::parse("foo").satisfies("foo"));
        assert!(PackageRelation::parse("foo (>= 1)").satisfies("foo"));
        assert!(!PackageRelation::parse("foo").satisfies("foo (>= 1)"));
        assert!(PackageRelation::parse("foo, bar").satisfies("foo"));
        assert!(PackageRelation::parse("foo, bar").satisfies("foo, bar"));
        assert!(!PackageRelation::parse("foo").satisfies("foo, bar"));
        assert!(!PackageRelation::parse("foo | bar").satisfies("foo"));
        assert!(PackageRelation::parse("foo | bar").satisfies("foo | bar | baz"));
        assert!(!PackageRelation::parse("foo | baz").satisfies("foo | bar"));
        assert!(PackageRelation::parse("debhelper (>= 13)").satisfies("debhelper (>= 13~)"));
        assert!(PackageRelation::parse("debhelper (>= 9)").satisfies("debhelper (>= 9~)"));
        assert!(!PackageRelation::parse("debhelper").satisfies("debhelper (>= 9~)"));
        assert!(!PackageRelation::parse("debhelper (>= 13)").satisfies("debhelper (>= 13.15~)"));
        assert!(PackageRelation::parse("debhelper (>= 14)").satisfies("debhelper (>= 13.15~)"));
        assert!(PackageRelation::parse("foo (= 1.0)").satisfies("foo (= 1.00)"));
        assert!(PackageRelation::parse("foo (= 1.0)").satisfies("foo (>= 1.0)"));
        assert!(!PackageRelation::parse("foo (= 1.0)").satisfies("foo (>= 1.1)"));
        assert!(PackageRelation::parse("foo (>= 2)").satisfies("foo (>= 1)"));
        assert!(!PackageRelation::parse("foo (>= 1)").satisfies("foo (>= 2)"));
        assert!(PackageRelation::parse("foo (<< 1)").satisfies("foo (<< 2)"));
        assert!(PackageRelation::parse("foo (<< 1)").satisfies("foo (<< 1)"));
        assert!(!PackageRelation::parse("foo (<< 2)").satisfies("foo (<< 1)"));
        assert!(PackageRelation::parse("foo (>> 1)").satisfies("foo (>> 1)"));
        assert!(PackageRelation::parse("foo (<< 1)").satisfies("foo (<= 1)"));
        assert!(!PackageRelation::parse("foo (<= 2)").satisfies("foo (<< 2)"));
        assert!(PackageRelation::parse("foo (>> 1)").satisfies("foo (>= 1)"));
        assert!(!PackageRelation::parse("foo (>= 1)").satisfies("foo (>> 1)"));
        assert!(PackageRelation::parse("").satisfies(""));
        assert!(!PackageRelation::parse("").satisfies("foo"));
        assert!(PackageRelation::parse("foo").satisfies(""));
    }

    #[test]
    fn satisfies_restrictions_qualifiers_and_substvars() {
        assert!(!PackageRelation::parse("foo [amd64]").satisfies("foo"));
        assert!(PackageRelation::parse("foo [amd64 i386]").satisfies("foo [amd64]"));
        assert!(!PackageRelation::parse("foo [amd64]").satisfies("foo [amd64 i386]"));
        assert!(
            PackageRelation::parse("debhelper (>= 13) [amd64]")
                .satisfies("debhelper (>= 13~) [amd64]")
        );
        assert!(!PackageRelation::parse("foo <!nocheck>").satisfies("foo"));
        assert!(!PackageRelation::parse("foo").satisfies("foo <!nocheck>"));
        assert!(PackageRelation::parse("libfoo").satisfies("libfoo:any"));
        assert!(!PackageRelation::parse("libfoo:any").satisfies("libfoo"));
        assert!(!PackageRelation::parse("libfoo").satisfies("libfoo:amd64"));
        assert!(PackageRelation::parse("libfoo:any").satisfies("libfoo:any"));
        assert!(PackageRelation::parse("${misc:Depends}").satisfies("${misc:Depends}"));
        assert!(PackageRelation::parse("foo, ${misc:Depends}").satisfies("${misc:Depends}"));
        assert!(!PackageRelation::parse("foo | ${misc:Depends}").satisfies("${misc:Depends}"));
        assert!(PackageRelation::parse("node-foo (= 1.0-1)").satisfies("node-foo"));
        let provides =
            PackageRelation::parse("node-bar").and(&PackageRelation::parse("node-foo (= 1.0-1)"));
        assert!(provides.satisfies("node-foo"));
        assert!(provides.satisfies("node-bar"));
        assert!(!PackageRelation::parse("node-foo | node-bar").satisfies("node-foo"));
    }

    #[test]
    fn restriction_less_drops_versions_architectures_and_profiles() {
        let relation = PackageRelation::parse(
            "debhelper (>= 13) [amd64], foo <!nocheck>, bar:native | baz [!i386] <!stage1>, , q [",
        )
        .restriction_less();
        assert_eq!(
            relation.to_lintian_string(),
            "debhelper, foo, bar | baz, q ["
        );
        assert!(relation.satisfies("debhelper | debhelper-compat"));
        assert!(relation.satisfies("foo"));
        assert!(relation.satisfies("foo:any"));
        assert!(!relation.satisfies("bar"));
        assert!(relation.satisfies("bar | baz"));
        // The version is gone, so a versioned condition is no longer implied.
        assert!(!relation.satisfies("debhelper (>= 9)"));
    }

    #[test]
    fn lintian_string_keeps_substvars_and_version_spacing() {
        let relation = PackageRelation::parse(
            "relations (<< 3), dpkg, bash (>> 2.0 ), mail-transport-agent, gawk | awk, foo (>> 2.0), foo (<< 2.2), coreutils, null (>= 0), ${misc:Depends}, makedev",
        );
        assert_eq!(
            relation.to_lintian_string(),
            "relations (<< 3), dpkg, bash (>> 2.0 ), mail-transport-agent, gawk | awk, foo (>> 2.0), foo (<< 2.2), coreutils, null (>= 0), ${misc:Depends}, makedev"
        );
        assert!(relation.satisfies("relations"));
        let unparsed = PackageRelation::parse(
            "control-file-general, foo, baz, ${shlibs:Depends} ${some:Depends}, ${misc:Depends}",
        );
        assert_eq!(
            unparsed.to_lintian_string(),
            "control-file-general, foo, baz, ${shlibs:Depends} ${some:Depends}, ${misc:Depends}"
        );
    }
}
