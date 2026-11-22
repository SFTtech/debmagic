use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::lines::{is_blank_or_comment, perl_lines, symbols_text};
use super::symbols;

pub struct SymbolsFileMissingBuildDependsPackageField;

declare_rule! {
    /// A SONAME in the `symbols` control file has no `Build-Depends-Package`
    /// or `Build-Depends-Packages` meta field.
    ///
    /// Every SONAME header line (its soname, then a package name, then an
    /// optional `(relation)` or `#MINVER#`) owns the `* <label>: <value>`
    /// meta lines below it, up to the next header. Each distinct SONAME, in
    /// file order, that owns neither label is reported once, with the SONAME
    /// as Extra and the Location `symbols` without a line. Applies to deb,
    /// udeb and ddeb. A `symbols` that is a symlink reads as empty, and one
    /// that is not UTF-8 reads as empty.
    SymbolsFileMissingBuildDependsPackageField,
    code = "LN1334",
    tag = "symbols-file-missing-build-depends-package-field",
    default_selected = false,
    experimental = false,
}

/// The meta labels that name the packages to build-depend on.
const BUILD_DEPENDS_LABELS: [&str; 2] = ["Build-Depends-Package", "Build-Depends-Packages"];

/// A soname header line: the soname, then its package name.
static SONAME_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname header")
});

/// A meta line: `* <label>: <value>`.
static META_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\*\s(\S+):\s\S+").expect("meta line"));

/// The SONAMEs in file order (each once), and the meta labels each one owns.
struct Headers {
    sonames: Vec<String>,
    labels: HashMap<String, HashSet<String>>,
}

fn parse(text: &str) -> Headers {
    let mut headers = Headers {
        sonames: Vec::new(),
        labels: HashMap::new(),
    };
    let mut seen = HashSet::new();
    let mut current: Option<String> = None;
    for line in perl_lines(text) {
        if is_blank_or_comment(line) {
            continue;
        }
        if let Some(captures) = SONAME_HEADER.captures(line) {
            let soname = captures[1].to_string();
            if seen.insert(soname.clone()) {
                headers.sonames.push(soname.clone());
            }
            current = Some(soname);
            continue;
        }
        let (Some(captures), Some(soname)) = (META_LINE.captures(line), &current) else {
            continue;
        };
        headers
            .labels
            .entry(soname.clone())
            .or_default()
            .insert(captures[1].to_string());
    }
    headers
}

impl BinaryPackageRule for SymbolsFileMissingBuildDependsPackageField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = symbols_text(ctx) else {
            return;
        };
        let headers = parse(&text);
        for soname in &headers.sonames {
            let has_build_depends = headers.labels.get(soname).is_some_and(|labels| {
                BUILD_DEPENDS_LABELS
                    .iter()
                    .any(|label| labels.contains(*label))
            });
            if !has_build_depends {
                ctx.diagnostic(
                    Diagnostic::warn(soname.as_str())
                        .with_severity(Severity::Info)
                        .with_location(symbols::at(None)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn symbols(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file("symbols", text)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            symbols("libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: libfoo-dev\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Build-Depends-Packages: libfoo-dev\n"),
            // Other labels do not count.
            symbols(
                "libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: libfoo-dev\n* Ignore-Blacklist-Groups: x\n",
            ),
            // The label belongs to its own SONAME.
            symbols(
                "libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: libfoo-dev\nlibbar.so.1 libbar1 1.0\n * Build-Depends-Package: x\n* Build-Depends-Package: y\n",
            ),
            // A symbols file with no SONAME.
            symbols("# comment\n* Build-Depends-Package: libfoo-dev\n"),
        ];
        let fail = vec![
            symbols("libfoo.so.1 libfoo1 1.0\n a 1\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Ignore-Blacklist-Groups: x\n"),
            // A meta line above the first header belongs to no SONAME.
            symbols("* Build-Depends-Package: libfoo-dev\nlibfoo.so.1 libfoo1 1.0\n"),
            // A SONAME declared twice is reported once.
            symbols("libfoo.so.1 libfoo1 1.0\nlibfoo.so.1 libfoo1 1.0\n"),
            // One SONAME is covered, the other is not.
            symbols(
                "libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: libfoo-dev\nlibbar.so.1 libbar1 1.0\n",
            ),
        ];
        BinaryPackageTester::new(SymbolsFileMissingBuildDependsPackageField, pass, fail)
            .test_and_snapshot();
    }
}
