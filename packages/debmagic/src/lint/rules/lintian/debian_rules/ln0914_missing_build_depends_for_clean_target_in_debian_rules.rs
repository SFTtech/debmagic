use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree};

pub struct MissingBuildDependsForCleanTargetInDebianRules;

declare_rule! {
    /// `debian/rules` needs a prerequisite for `clean` that only
    /// `Build-Depends-Indep` provides.
    ///
    /// Walks `debian/rules` as lintian's `debian/rules` Check does: comment
    /// lines are dropped, `\` continuations joined, and lines inside
    /// `ifeq`, `ifneq`, `ifdef`, or `ifndef` blocks are skipped for the
    /// global tests. Needed for the clean target are `gnome-pkg-tools | dh-sequence-gnome`,
    /// `quilt`, `mozilla-devscripts`, and `ruby-pkg-tools` when their include
    /// appears outside a conditional; and, in a recipe of an arch target
    /// (`clean`, `binary-arch`, `build-arch`, or a target one of them depends
    /// on), `ant`, `debhelper | debhelper-compat` for `dh_*` other than
    /// `dh_autoreconf`, `dh-ocaml, ocaml-nox | ocaml` for `dh_ocamlinit`,
    /// `po-debconf` for `debconf-updatepo`, `quilt`, python3 for `python3`,
    /// and python3 for `setup.py`. A prerequisite that `Build-Depends`
    /// satisfies is not needed. Reported for each needed prerequisite that
    /// `Build-Depends-Indep` satisfies. Extra is `(does not satisfy <prerequisite>)`,
    /// with lintian's wording. Location is `debian/rules` with no line.
    /// Applies to the Source tree and Source package. Nothing is reported
    /// without a `debian/control` that parses, or without a readable
    /// `debian/rules`. A symlinked `debian/rules` is not followed for its
    /// contents (lintian follows it).
    MissingBuildDependsForCleanTargetInDebianRules,
    code = "LN0914",
    tag = "missing-build-depends-for-clean-target-in-debian-rules",
    default_selected = true,
    experimental = false,
}

/// lintian 2.144.0 `@arch_rules` before any dependency is added.
const ARCH_RULES: &[&str] = &["clean", "binary-arch", "build-arch"];

const PYTHON3_DEPEND: &str =
    "python3:any | python3-dev:any | python3-all:any | python3-all-dev:any";
const ANYPYTHON_DEPEND: &str = "python3:any | python3-dev:any | python3-all:any | python3-all-dev:any | python3.13:any | python3.13-dev:any | python3.14:any | python3.14-dev:any";

static CONDITIONAL_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("valid regex"));
static CONDITIONAL_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^endif\s").expect("valid regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^([^\s:][^:]*):+(.*)").expect("valid regex"));
static PHONY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("valid regex")
});
static RECIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("valid regex"));
static DEBHELPER_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\t\s*dh_(.+)").expect("valid regex"));

/// A `debian/rules` line test. The debhelper test is `dh_(?!autoreconf).+`,
/// whose lookahead the regex crate does not offer.
enum LineTest {
    Regex(Regex),
    DebhelperCommand,
}

impl LineTest {
    fn regex(pattern: &str) -> Self {
        Self::Regex(Regex::new(pattern).expect("valid regex"))
    }

    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(line),
            Self::DebhelperCommand => DEBHELPER_COMMAND
                .captures(line)
                .is_some_and(|captures| !captures[1].starts_with("autoreconf")),
        }
    }
}

struct Prerequisite {
    key: &'static str,
    test: LineTest,
}

struct Tables {
    global_clean: Vec<Prerequisite>,
    rule_clean: Vec<Prerequisite>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    global_clean: vec![
        Prerequisite {
            key: "gnome-pkg-tools | dh-sequence-gnome",
            test: LineTest::regex(r"^include\s+/usr/share/gnome-pkg-tools/"),
        },
        Prerequisite {
            key: "quilt",
            test: LineTest::regex(r"^include\s+/usr/share/quilt/"),
        },
        Prerequisite {
            key: "mozilla-devscripts",
            test: LineTest::regex(r"^include\s+/usr/share/mozilla-devscripts/"),
        },
        Prerequisite {
            key: "ruby-pkg-tools",
            test: LineTest::regex(r"^include\s+/usr/share/ruby-pkg-tools/1/class/"),
        },
    ],
    rule_clean: vec![
        Prerequisite {
            key: "ant",
            test: LineTest::regex(r"^\t\s*(\S+=\S+\s+)*ant\s"),
        },
        Prerequisite {
            key: "debhelper | debhelper-compat",
            test: LineTest::DebhelperCommand,
        },
        Prerequisite {
            key: "dh-ocaml, ocaml-nox | ocaml",
            test: LineTest::regex(r"^\t\s*dh_ocamlinit\s"),
        },
        Prerequisite {
            key: "po-debconf",
            test: LineTest::regex(r"^\t\s*debconf-updatepo\s"),
        },
        Prerequisite {
            key: PYTHON3_DEPEND,
            test: LineTest::regex(r"^\t\s*python3\s"),
        },
        Prerequisite {
            key: ANYPYTHON_DEPEND,
            test: LineTest::regex(r"\ssetup\.py\b"),
        },
        Prerequisite {
            key: "quilt",
            test: LineTest::regex(r"^\t\s*(\S+=\S+\s+)*quilt\s"),
        },
    ],
});

impl SourceRule for MissingBuildDependsForCleanTargetInDebianRules {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        let (build_depends, build_depends_indep) = ctx
            .debian_control()
            .map(|control| {
                (
                    control
                        .source()
                        .relation("Build-Depends")
                        .unwrap_or_default(),
                    control
                        .source()
                        .relation("Build-Depends-Indep")
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        for prerequisite in needed_clean(&contents, &build_depends) {
            if !build_depends_indep.satisfies(prerequisite) {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::error(format!("(does not satisfy {prerequisite})")).with_location(
                    Location {
                        path: PathBuf::from("debian/rules"),
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// Prerequisite keys the clean target needs, in first-seen order. A key that
/// `Build-Depends` already satisfies is left out, as lintian drops it before
/// the walk.
fn needed_clean(contents: &str, build_depends: &PackageRelation) -> Vec<&'static str> {
    let tables = &*TABLES;
    let mut needed = Vec::new();
    let mut arch_rules: Vec<Vec<Piece>> = ARCH_RULES
        .iter()
        .map(|name| vec![Piece::Text((*name).to_string())])
        .collect();
    let mut current: Vec<String> = Vec::new();
    // Perl's `$maybe_skipping`: a negative value is still true, so it is signed.
    let mut skipping = 0i32;

    for (_, line) in complete_lines(contents) {
        let opens = CONDITIONAL_OPEN.is_match(&line);
        if opens {
            skipping += 1;
        } else if CONDITIONAL_CLOSE.is_match(&line) {
            skipping -= 1;
        }

        if skipping == 0 {
            for prerequisite in &tables.global_clean {
                if prerequisite.test.is_match(&line) {
                    push_unique(&mut needed, prerequisite.key, build_depends);
                }
            }
        }

        // A `.PHONY` line is not a target for lintian's walk.
        if PHONY.is_match(&line) {
            continue;
        }

        if !opens && let Some(captures) = TARGET.captures(&line) {
            current = captures[1].split_whitespace().map(str::to_string).collect();
            let dependencies: Vec<&str> = captures[2].split_whitespace().collect();
            for target in &current {
                if arch_rules.iter().any(|pattern| matches(pattern, target)) {
                    arch_rules.extend(dependencies.iter().map(|word| word_pattern(word)));
                }
            }
        } else if line.starts_with("define ") {
            current.clear();
        } else if RECIPE.is_match(&line) {
            // Only an arch recipe needs the clean prerequisites; lintian also
            // walks indep recipes, but they add nothing to the clean list.
            let arch = current
                .iter()
                .any(|target| arch_rules.iter().any(|pattern| matches(pattern, target)));
            if skipping == 0 && arch {
                for prerequisite in &tables.rule_clean {
                    if prerequisite.test.is_match(&line) {
                        push_unique(&mut needed, prerequisite.key, build_depends);
                    }
                }
            }
        }
    }
    needed
}

fn push_unique(needed: &mut Vec<&'static str>, key: &'static str, build_depends: &PackageRelation) {
    if !build_depends.satisfies(key) && !needed.contains(&key) {
        needed.push(key);
    }
}

/// A `debian/rules` word as lintian matches it against a target name: literal
/// text, or a `.*` wildcard where a `$(VAR:from=to)` reference stood.
#[derive(Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Any,
}

/// Lintian's `$(VAR:a=b)` handling: the reference becomes `b` (less the
/// trailing `a`) followed by `.*`, and the word is anchored at both ends.
fn word_pattern(word: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut rest = word;
    while let Some((start, end, to)) = find_reference(rest) {
        pieces.push(Piece::Text(rest[..start].to_string()));
        pieces.push(Piece::Text(to.to_string()));
        pieces.push(Piece::Any);
        rest = &rest[end..];
    }
    pieces.push(Piece::Text(rest.to_string()));
    pieces.retain(|piece| !matches!(piece, Piece::Text(text) if text.is_empty()));
    pieces
}

/// First `$(VAR:from=to)` reference in `text`: its byte range and `to`.
fn find_reference(text: &str) -> Option<(usize, usize, &str)> {
    let mut search = 0;
    while let Some(offset) = text[search..].find("$(") {
        let start = search + offset;
        if let Some((consumed, to)) = parse_reference(&text[start + 2..]) {
            return Some((start, start + 2 + consumed, to));
        }
        search = start + 1;
    }
    None
}

/// Parses `VAR:from=to)` after `$(`. Returns the bytes consumed through the
/// closing parenthesis and the `to` text without the trailing `from`.
fn parse_reference(inner: &str) -> Option<(usize, &str)> {
    let colon = inner.find([':', ')'])?;
    if colon == 0 || !inner[colon..].starts_with(':') {
        return None;
    }
    let after = &inner[colon + 1..];
    let equals = after.find('=')?;
    let from = &after[..equals];
    if from.is_empty() {
        return None;
    }
    let rest = &after[equals + 1..];
    let close = rest.find(')')?;
    let to = rest[..close].strip_suffix(from)?;
    if to.is_empty() {
        return None;
    }
    Some((colon + 1 + equals + 1 + close + 1, to))
}

fn matches(pattern: &[Piece], name: &str) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((Piece::Text(text), rest)) => name
            .strip_prefix(text.as_str())
            .is_some_and(|tail| matches(rest, tail)),
        Some((Piece::Any, rest)) => name
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(name.len()))
            .any(|index| matches(rest, &name[index..])),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(build_depends: Option<&str>, build_depends_indep: Option<&str>) -> String {
        let mut text = String::from(
            "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n",
        );
        if let Some(value) = build_depends {
            text.push_str(&format!("Build-Depends: {value}\n"));
        }
        if let Some(value) = build_depends_indep {
            text.push_str(&format!("Build-Depends-Indep: {value}\n"));
        }
        text.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        text
    }

    fn package(
        build_depends: Option<&str>,
        build_depends_indep: Option<&str>,
        body: &str,
    ) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(control(build_depends, build_depends_indep)),
            ),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n%:\n\tdh $@\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(None, None, ""),
            // Build-Depends satisfies the clean prerequisite.
            package(
                Some("debhelper-compat (= 13)"),
                Some("debhelper-compat (= 13)"),
                "clean:\n\tdh_clean\n",
            ),
            // An indep recipe needs no clean prerequisite.
            package(None, Some("debhelper-compat"), "build-indep:\n\tdh_clean\n"),
            // Without Build-Depends-Indep lintian has nothing to say about it here.
            package(None, None, "clean:\n\tdh_clean\n"),
            // dh_autoreconf is not a debhelper prerequisite for the clean target.
            package(None, Some("debhelper-compat"), "clean:\n\tdh_autoreconf\n"),
            // A comment is not a recipe.
            package(None, Some("debhelper-compat"), "clean:\n\t# dh_clean\n"),
            // A conditional hides the clean recipe from the walk.
            package(
                None,
                Some("debhelper-compat"),
                "ifeq (a,b)\nclean:\n\tdh_clean\nendif\n",
            ),
        ];
        let fail = vec![
            package(
                None,
                Some("debhelper-compat (= 13)"),
                "clean:\n\tdh_clean\n",
            ),
            // Build-Depends names a different package, so debhelper is still missing.
            package(
                Some("quilt"),
                Some("debhelper-compat"),
                "clean:\n\tdh_clean\n",
            ),
            // The clean target's dependency makes the arch recipe count.
            package(
                None,
                Some("debhelper-compat"),
                "clean: build-stamp\nbuild-stamp:\n\tdh_clean\n",
            ),
            // A global include that needs quilt in the clean target.
            package(None, Some("quilt"), "include /usr/share/quilt/quilt.make\n"),
        ];
        SourceTreeTester::new(MissingBuildDependsForCleanTargetInDebianRules, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            Some("debhelper-compat"),
            Some("debhelper-compat"),
            "clean:\n\tdh_clean\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            None,
            Some("debhelper-compat"),
            "clean:\n\tdh_clean\n",
        ))];
        SourcePackageTester::new(MissingBuildDependsForCleanTargetInDebianRules, pass, fail)
            .test_and_snapshot();
    }
}
