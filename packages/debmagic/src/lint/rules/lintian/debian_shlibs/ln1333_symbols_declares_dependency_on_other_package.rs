use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{is_blank_or_comment, perl_lines, symbols_text};
use super::symbols;

pub struct SymbolsDeclaresDependencyOnOtherPackage;

declare_rule! {
    /// The `symbols` control file declares a dependency the package does not
    /// satisfy.
    ///
    /// Dependencies come from the soname line (after the soname and its
    /// package name) and from the alternative lines (`| pkg`). Each
    /// dependency is split on `,` and `|`; a term that is a package name with
    /// an optional `(relation)` or `#MINVER#` is a dependency, and a `#MINVER#`
    /// suffix is dropped. Per SONAME, in file order, each distinct raw
    /// dependency is checked once against the package's `Provides` AND-ed with
    /// its own name (plus `(= Version)` when `Version` is declared). An
    /// unsatisfied one is reported with extra `<dependency> (<soname>)` and the
    /// Location `symbols` with no line. Applies to deb, udeb and ddeb. A
    /// `symbols` that is a symlink reads as empty. A term that does not parse
    /// as a package name with an optional version is skipped.
    SymbolsDeclaresDependencyOnOtherPackage,
    code = "LN1333",
    tag = "symbols-declares-dependency-on-other-package",
    default_selected = true,
    experimental = false,
}

/// A soname line: the soname, then its package name, then an optional
/// `(relation)` or `#MINVER#` (lintian's soname template line).
static SONAME_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname line")
});

/// An alternative template line: `| package` with an optional version.
static ALTERNATIVE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\|\s+\S+\s*(?:\([^\s]+\s+[^\s]+\)|#MINVER#)?").expect("alternative line")
});

/// One dependency term: a package name, then an optional version.
static DEPENDENCY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\S+)(\s*(?:\(\S+\s+\S+\)|#MINVER#))?$").expect("dependency term")
});

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("comma"));
static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\|\s*").expect("pipe"));

/// Sonames in file order (repeats kept), and the dependencies of each soname
/// in the order they are first seen, raw (a `#MINVER#` suffix still attached).
struct Declared {
    sonames: Vec<String>,
    dependencies: HashMap<String, Vec<String>>,
}

fn parse(text: &str) -> Declared {
    let mut declared = Declared {
        sonames: Vec::new(),
        dependencies: HashMap::new(),
    };
    let mut current = String::new();
    for line in perl_lines(text) {
        if is_blank_or_comment(line) {
            continue;
        }
        if let Some(captures) = SONAME_LINE.captures(line) {
            current = captures[1].to_string();
            declared.sonames.push(current.clone());
            let rest = line[current.len()..].trim_start();
            add_dependencies(&mut declared.dependencies, &current, rest);
        } else if ALTERNATIVE_LINE.is_match(line) {
            let rest = line.strip_prefix('|').unwrap_or(line).trim_start();
            add_dependencies(&mut declared.dependencies, &current, rest);
        }
    }
    declared
}

fn add_dependencies(map: &mut HashMap<String, Vec<String>>, soname: &str, text: &str) {
    for conjunctive in COMMA.split(text) {
        for disjunctive in PIPE.split(conjunctive) {
            let Some(captures) = DEPENDENCY.captures(disjunctive) else {
                continue;
            };
            let version = captures.get(2).map_or("", |m| m.as_str());
            map.entry(soname.to_string())
                .or_default()
                .push(format!("{}{version}", &captures[1]));
        }
    }
}

impl BinaryPackageRule for SymbolsDeclaresDependencyOnOtherPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = symbols_text(ctx) else {
            return;
        };
        let declared = parse(&text);
        let provides = provides_or_self(ctx);
        let mut seen_sonames = HashSet::new();
        for soname in &declared.sonames {
            if !seen_sonames.insert(soname.as_str()) {
                continue;
            }
            let Some(dependencies) = declared.dependencies.get(soname) else {
                continue;
            };
            let mut seen = HashSet::new();
            for dependency in dependencies {
                if !seen.insert(dependency.as_str()) {
                    continue;
                }
                let condition = dependency.strip_suffix(" #MINVER#").unwrap_or(dependency);
                if provides.satisfies(condition) {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::warn(format!("{condition} ({soname})"))
                        .with_location(symbols::at(None)),
                );
            }
        }
    }
}

/// Lintian's `Provides` AND the package itself, with `(= Version)` when the
/// installation control declares a Version.
fn provides_or_self(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    let fields = ctx.installation_control().fields();
    let own = if fields.declares("Version") {
        format!(
            "{} (= {})",
            ctx.package_name(),
            fields.unfolded_value("Version").unwrap_or_default()
        )
    } else {
        ctx.package_name().to_string()
    };
    fields
        .relation("Provides")
        .unwrap_or_default()
        .and(&PackageRelation::parse(&own))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn symbols(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .package("libfoo1")
            .control_file("symbols", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            // No symbols file.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package("libfoo1"),
            // The package's own name, with and without `#MINVER#`.
            symbols("libfoo.so.1 libfoo1 #MINVER#\n| libfoo1\n"),
            symbols("libfoo.so.1 libfoo1\n"),
            // A Provides satisfies the dependency.
            symbols("libfoo.so.1 libbar\n").field("Provides", "libbar"),
            // `(= Version)` of the package itself.
            symbols("libfoo.so.1 libfoo1 (= 1-1)\n").field("Version", "1-1"),
            // A repeated dependency is checked once, and a satisfied one is quiet.
            symbols("libfoo.so.1 libfoo1\nlibfoo.so.1 libfoo1\n"),
            // A comment and blank lines are skipped.
            symbols("# comment\n\nlibfoo.so.1 libfoo1\n"),
        ];
        let fail = vec![
            symbols("libfoo.so.1 libbar\n"),
            symbols("libfoo.so.1 libbar (>= 1)\n"),
            // An alternative line is a dependency of the current soname.
            symbols("libfoo.so.1 libfoo1\n| libbar (>= 1)\n"),
            // A dependency under two sonames is reported under each.
            symbols("libfoo.so.1 libbar\nlibfoo.so.2 libbar\n"),
            // The Version does not satisfy another package's version.
            symbols("libfoo.so.1 libfoo1 (= 2-1)\n").field("Version", "1-1"),
            // A dependency with `#MINVER#` is reported without it.
            symbols("libfoo.so.1 libbar #MINVER#\n"),
        ];
        BinaryPackageTester::new(SymbolsDeclaresDependencyOnOtherPackage, pass, fail)
            .test_and_snapshot();
    }
}
