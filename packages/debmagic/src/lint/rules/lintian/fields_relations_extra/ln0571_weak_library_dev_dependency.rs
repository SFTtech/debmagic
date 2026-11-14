use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{comma_list, is_arch_all};

pub struct WeakLibraryDevDependency;

declare_rule! {
    /// A `-dev` package does not tightly depend on the library package it ships with.
    ///
    /// For each installable named `*-dev` that declares `Depends`, every other
    /// installable named like a versioned library (`lib...<digit>`, not
    /// `-dev`, `-doc`, `-docs`, or `-common`) is looked for among the
    /// comma-separated `Depends` entries. Nothing is reported when an entry
    /// has alternatives, or when there are no entries or more than two. One
    /// entry must be `= ${binary:Version}` (or `${Source-Version}`); an
    /// `= ${source:Version}` entry is accepted when the library is
    /// `Architecture: all`. Two entries must be an upper bound (`<<` or `<=`)
    /// and a lower bound (`>>` or `>=`) on a binary, source, or upstream
    /// version substvar. Extra is `(in section for <dev package>)`, `Depends`,
    /// and the offending entry or entries.
    WeakLibraryDevDependency,
    code = "LN0571",
    tag = "weak-library-dev-dependency",
    default_selected = true,
    experimental = false,
}

static LIBRARY_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lib[\w.+-]+\d").expect("valid regex"));
static EXCLUDED_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-(?:dev|docs?|common)$").expect("valid regex"));
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\w.+-]+\s*[(]([^)]+)[)]").expect("valid regex"));
static TIGHT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*=\s*\$\{(?:binary:Version|Source-Version)\}").expect("valid regex")
});
static ALL_SOURCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*=\s*\$[{]source:Version[}]").expect("valid regex"));
static UPPER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*<[=<]\s*\$[{](?:(?:binary|source):(?:Upstream-)?Version|Source-Version)[}]")
        .expect("valid regex")
});
static LOWER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*>[=>]\s*\$[{](?:(?:binary|source):(?:Upstream-)?Version|Source-Version)[}]")
        .expect("valid regex")
});

impl SourceRule for WeakLibraryDevDependency {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx.debian_control().map(messages).unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(control: &DebianControl) -> Vec<String> {
    let mut hits = Vec::new();
    for dev in control.installables() {
        if !dev.name().ends_with("-dev") || !dev.fields().declares("Depends") {
            continue;
        }
        let depends = comma_list(dev.fields(), "Depends");
        for other in control.installables() {
            let other_name = other.name();
            if EXCLUDED_NAME.is_match(other_name) || !LIBRARY_NAME.is_match(other_name) {
                continue;
            }
            let mention = Regex::new(&format!(
                r"(?:^|[\s|]){}(?:[\s|(]|$)",
                regex::escape(other_name)
            ))
            .expect("valid regex");
            let relevant: Vec<&String> = depends.iter().filter(|d| mention.is_match(d)).collect();
            if relevant.iter().any(|entry| entry.contains('|')) {
                continue;
            }
            let mut versions: Vec<&str> = relevant
                .iter()
                .map(|entry| {
                    VERSION
                        .captures(entry)
                        .and_then(|captures| captures.get(1))
                        .map_or("", |version| version.as_str())
                })
                .collect();
            versions.sort_unstable();
            let context = match relevant.len() {
                1 => {
                    if TIGHT.is_match(versions[0])
                        || (is_arch_all(other.fields()) && ALL_SOURCE.is_match(versions[0]))
                    {
                        None
                    } else {
                        Some(relevant[0].to_string())
                    }
                }
                2 => {
                    if UPPER.is_match(versions[0]) && LOWER.is_match(versions[1]) {
                        None
                    } else {
                        Some(format!("{}, {}", relevant[0], relevant[1]))
                    }
                }
                _ => None,
            };
            if let Some(context) = context {
                hits.push(format!("(in section for {}) Depends {context}", dev.name()));
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::raw;
    use super::*;

    fn dev(depends: &str, library_architecture: &str) -> Vec<(&'static str, TestFile)> {
        raw(&format!(
            "Source: example\n\nPackage: libexample1\nArchitecture: {library_architecture}\n\n\
             Package: libexample-dev\nDepends: {depends}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            // Not a -dev package.
            raw(
                "Source: example\n\nPackage: libexample1\n\nPackage: libexample-data\nDepends: libexample1\n",
            ),
            // No Depends at all.
            raw("Source: example\n\nPackage: libexample1\n\nPackage: libexample-dev\n"),
            dev("libexample1 (= ${binary:Version}), libc6-dev", "any"),
            dev("libexample1 (= ${Source-Version})", "any"),
            dev("libexample1 (= ${source:Version})", "all"),
            dev(
                "libexample1 (>= ${source:Version}), libexample1 (<< ${source:Version}.1)",
                "any",
            ),
            dev(
                "libexample1 (<= ${binary:Version}), libexample1 (>> ${source:Upstream-Version})",
                "any",
            ),
            // An alternative means the maintainer knows better.
            dev("libexample1 | libother1", "any"),
            // Not mentioned, or three mentions.
            dev("libc6", "any"),
            dev("libexample1, libexample1 (>= 1), libexample1 (<< 2)", "any"),
            // A name without a digit is not a versioned library.
            raw(
                "Source: example\n\nPackage: libexample\n\nPackage: libexample-dev\nDepends: libexample\n",
            ),
        ];
        let fail = vec![
            dev("libexample1", "any"),
            dev("libexample1 (= ${source:Version})", "any"),
            dev("libexample1 (>= ${binary:Version})", "all"),
            dev(
                "libexample1 (>= ${binary:Version}), libexample1 (>= ${binary:Version})",
                "any",
            ),
            dev("libexample1 (<< 2), libexample1 (>= 1)", "any"),
            raw("Source: example\n\n\
                 Package: libexample1\n\n\
                 Package: libexample2\nArchitecture: all\n\n\
                 Package: libexample-dev\nDepends: libexample1 (= ${binary:Version}),\n libexample2 (>= ${source:Version})\n"),
        ];
        SourceTreeTester::new(WeakLibraryDevDependency, pass, fail).test_and_snapshot();
    }
}
