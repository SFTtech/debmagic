use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::super::rules_and_control::{read_rules, rules_lines};

pub struct RulesRequireBuildPrerequisite;

declare_rule! {
    /// `debian/rules` needs a build prerequisite that Build-Depends lacks.
    ///
    /// Walks `debian/rules` the way lintian's `debian/rules` Check does: comment
    /// lines are dropped and `\` continuations joined. Lines inside make
    /// conditionals (`ifeq`, `ifneq`, `ifdef`, `ifndef` up to `endif`) are
    /// skipped for the global and recipe tests. Needed are `debhelper` or
    /// `debhelper-compat` for `dh_*` commands other than `dh_autoreconf`,
    /// `dh-autoreconf` for `dh_autoreconf`, the `dh-ocaml`, `gnome-pkg-tools`,
    /// `quilt`, `mozilla-devscripts`, and `ruby-pkg-tools` prerequisites when
    /// their include or command appears, and `python3` or `python3.X` for
    /// `python3` and `setup.py` in a recipe of an arch or indep target
    /// (`clean`, `build-arch`, `binary-indep`, a `%` pattern, and targets an
    /// arch target depends on). Reported when `Build-Depends`,
    /// `Build-Depends-Indep`, and `Build-Depends-Arch` together, with versions,
    /// architecture restrictions, and build profiles dropped, do not imply the
    /// prerequisite. Extra is `(does not satisfy` the prerequisite `)`, each
    /// alternative qualified with `:any`. Location is `debian/rules` with no
    /// line. Applies to the Source tree and Source package.
    RulesRequireBuildPrerequisite,
    code = "LN0712",
    tag = "rules-require-build-prerequisite",
    default_selected = true,
    experimental = false,
}

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

/// lintian 2.139.0 `@indep_rules`.
const INDEP_RULES: &[&str] = &["build", "build-indep", "binary-indep"];

/// lintian 2.139.0 `@arch_rules` before any dependency is added.
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
    global_depends: Vec<Prerequisite>,
    global_clean: Vec<Prerequisite>,
    rule_clean: Vec<Prerequisite>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    global_depends: vec![
        Prerequisite {
            key: "dh-ocaml, ocaml-nox | ocaml",
            test: LineTest::regex(r"^\t\s*dh_ocaml(?:init|doc)\s"),
        },
        Prerequisite {
            key: "debhelper | debhelper-compat | dh-autoreconf",
            test: LineTest::regex(r"^\t\s*dh_autoreconf(?:_clean)?\s"),
        },
    ],
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

impl SourceRule for RulesRequireBuildPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules) else {
            return;
        };
        // lintian's `Build-Depends-All` without restrictions. Without a
        // control file the relation is empty, so every prerequisite is missing.
        let build_depends = ctx
            .debian_control()
            .map(|control| {
                control
                    .source()
                    .relations_and(BUILD_DEPENDS)
                    .restriction_less()
            })
            .unwrap_or_default();
        for key in needed_prerequisites(&contents) {
            let prerequisite = key.to_string();
            if build_depends.satisfies(&prerequisite) {
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

/// Prerequisite keys lintian's walk marks as needed, in first-seen order.
fn needed_prerequisites(contents: &str) -> Vec<&'static str> {
    let tables = &*TABLES;
    let mut needed = Vec::new();
    let mut arch_rules: Vec<Vec<Piece>> = ARCH_RULES
        .iter()
        .map(|name| vec![Piece::Text((*name).to_string())])
        .collect();
    let mut current: Vec<String> = Vec::new();
    // Perl's `$maybe_skipping`: a negative value is still true, so it is signed.
    let mut skipping = 0i32;

    for (_, line) in rules_lines(contents) {
        let opens = CONDITIONAL_OPEN.is_match(&line);
        if opens {
            skipping += 1;
        } else if CONDITIONAL_CLOSE.is_match(&line) {
            skipping -= 1;
        }

        if skipping == 0 {
            for prerequisite in tables.global_depends.iter().chain(&tables.global_clean) {
                if prerequisite.test.is_match(&line) {
                    push_unique(&mut needed, prerequisite.key);
                }
            }
        }

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
            let mut arch = false;
            let mut indep = false;
            for target in &current {
                arch |= arch_rules.iter().any(|pattern| matches(pattern, target));
                indep |= INDEP_RULES.contains(&target.as_str())
                    || target == "%"
                    || target.starts_with("override_");
            }
            if skipping == 0 && (arch || indep) {
                for prerequisite in &tables.rule_clean {
                    if prerequisite.test.is_match(&line) {
                        push_unique(&mut needed, prerequisite.key);
                    }
                }
            }
        }
    }
    needed
}

fn push_unique(needed: &mut Vec<&'static str>, key: &'static str) {
    if !needed.contains(&key) {
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

    fn control(build_depends: Option<&str>) -> String {
        let build_depends = build_depends
            .map(|value| format!("Build-Depends: {value}\n"))
            .unwrap_or_default();
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{build_depends}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    fn package_field(field: &str, value: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        let control =
            control(None).replace("\n\nPackage:", &format!("\n{field}: {value}\n\nPackage:"));
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    fn package(build_depends: Option<&str>, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(build_depends))),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(Some("debhelper (>= 9)"), "build-arch:\n\tdh_testdir\n"),
            package(
                Some("debhelper-compat (= 13)"),
                "binary-indep:\n\tdh_testdir\n",
            ),
            // A profile restriction is dropped, so the name still counts.
            package(Some("debhelper <!nocheck>"), "build-arch:\n\tdh_testdir\n"),
            // Only arch and indep targets need the helper's prerequisite.
            package(None, "custom:\n\tdh_testdir\n"),
            package(None, "# build-arch:\n\tdh_testdir\n"),
            // Make conditionals are skipped.
            package(None, "ifeq ($(X),y)\nbuild-arch:\n\tdh_testdir\nendif \n"),
            // Lintian's `endif\s` needs trailing whitespace, so a bare `endif`
            // leaves the conditional open for the rest of the file.
            package(None, "ifeq ($(X),y)\nendif\nbuild-arch:\n\tdh_testdir\n"),
            // `dh_autoreconf` is covered by dh-autoreconf.
            package(
                Some("dh-autoreconf"),
                "build-arch:\n\tdh_autoreconf --verbose\n",
            ),
            package(Some("python3-all"), "clean:\n\tpython3 setup.py clean\n"),
            // No python3 command or setup.py in an arch or indep recipe.
            package(None, "binary-indep:\n\techo python3\n"),
            // Build-Depends-Indep and Build-Depends-Arch count too.
            package_field(
                "Build-Depends-Indep",
                "debhelper",
                "build-indep:\n\tdh_testdir\n",
            ),
            package_field(
                "Build-Depends-Arch",
                "debhelper-compat (= 14)",
                "build-arch:\n\tdh_testdir\n",
            ),
        ];
        let fail = vec![
            package(None, "binary-indep:\n\tdh_testdir\n"),
            package_field("Build-Depends-Indep", "foo", "build-indep:\n\tdh_testdir\n"),
            package(Some("foo"), "build-arch:\n\tdh_testdir\n"),
            // A `%` pattern is an indep target, and autoreconf also needs
            // dh-autoreconf.
            package(None, "%:\n\tdh_autoreconf --verbose\n"),
            // Python: both the python3 and the setup.py prerequisite.
            package(None, "clean:\n\tpython3 setup.py clean\n"),
            // `clean: $(PYVERS:%=clean-python%)` makes `clean-python%` an arch rule.
            package(
                None,
                "clean: $(PYVERS:%=clean-python%)\nclean-python%:\n\tdh_testdir\n",
            ),
            // The conditional is closed, so the recipe is scanned again.
            package(None, "ifeq ($(X),y)\nendif \nbuild-arch:\n\tdh_testdir\n"),
        ];
        SourceTreeTester::new(RulesRequireBuildPrerequisite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            Some("debhelper"),
            "build-arch:\n\tdh_testdir\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            None,
            "build-arch:\n\tdh_testdir\n",
        ))];
        SourcePackageTester::new(RulesRequireBuildPrerequisite, pass, fail).test_and_snapshot();
    }
}
