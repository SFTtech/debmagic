use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownMetaFieldInSymbolsFile;

declare_rule! {
    /// The `symbols` control file has a meta-information line whose label is
    /// not one lintian knows: `Build-Depends-Package`, `Build-Depends-Packages`,
    /// or `Ignore-Blacklist-Groups`.
    ///
    /// Lintian's error, one Diagnostic per such line, with `<label> (<soname>)`
    /// as Extra and `symbols:<line>` as the Location. A meta line is `* <label>:
    /// <value>` (one space after `*`, a value required); it belongs to the
    /// SONAME of the last header line above it, and lines above the first
    /// header are not reported. A SONAME declared twice is reported once per
    /// line. A symbols file that is not UTF-8 is read as empty, as in lintian.
    /// Applies to deb, udeb, and ddeb.
    UnknownMetaFieldInSymbolsFile,
    code = "LN1041",
    tag = "unknown-meta-field-in-symbols-file",
    default_selected = true,
    experimental = false,
}

/// Meta labels lintian knows (`@known_meta_labels`).
const KNOWN_META_LABELS: [&str; 3] = [
    "Build-Depends-Package",
    "Build-Depends-Packages",
    "Ignore-Blacklist-Groups",
];

/// Lintian's soname header of a symbols file.
static SONAME_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname header")
});
static META_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\*\s(\S+):\s\S+").expect("meta line"));

impl BinaryPackageRule for UnknownMetaFieldInSymbolsFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = symbols_text(ctx) else {
            return;
        };
        // Headers in file order, and meta labels per SONAME with their lines.
        let mut sonames: Vec<String> = Vec::new();
        let mut metas: BTreeMap<String, BTreeMap<String, Vec<u32>>> = BTreeMap::new();
        let mut current: Option<String> = None;
        for (index, line) in text.split('\n').enumerate() {
            let line_number = index as u32 + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(captures) = SONAME_HEADER.captures(line) {
                let soname = captures[1].to_string();
                if !sonames.contains(&soname) {
                    sonames.push(soname.clone());
                }
                current = Some(soname);
                continue;
            }
            let Some(captures) = META_LINE.captures(line) else {
                continue;
            };
            let Some(soname) = &current else {
                continue;
            };
            metas
                .entry(soname.clone())
                .or_default()
                .entry(captures[1].to_string())
                .or_default()
                .push(line_number);
        }
        for soname in &sonames {
            let Some(labels) = metas.get(soname) else {
                continue;
            };
            for (label, lines) in labels {
                if KNOWN_META_LABELS.contains(&label.as_str()) {
                    continue;
                }
                for line in lines {
                    ctx.diagnostic(
                        Diagnostic::error(format!("{label} ({soname})")).with_location(Location {
                            path: "symbols".into(),
                            line: Some(*line),
                            column: None,
                        }),
                    );
                }
            }
        }
    }
}

/// The `symbols` control file as text; empty when it is not UTF-8. `None`
/// when there is no regular file of that name.
fn symbols_text(ctx: &mut BinaryPackageContext<'_>) -> Option<String> {
    let files = ctx.control_files()?;
    let bytes = files.read_file("symbols")?;
    Some(String::from_utf8(bytes).unwrap_or_default())
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
            symbols("libfoo.so.1 libfoo1 1.0\n a 1\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: libfoo-dev\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Build-Depends-Packages: libfoo-dev\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Ignore-Blacklist-Groups: x\n"),
            // Meta lines above the first header belong to no SONAME.
            symbols("* Bad: x\nlibfoo.so.1 libfoo1 1.0\n"),
            // Comments, a missing value, or indentation: not meta lines.
            symbols("# * Bad: x\nlibfoo.so.1 libfoo1 1.0\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Bad:\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n  * Bad: x\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "symbols",
                TestFile::from_bytes(b"libfoo.so.1 libfoo1 1.0\n* Bad: \xff\n".to_vec()),
            ),
        ];
        let fail = vec![
            symbols("libfoo.so.1 libfoo1 1.0\n* Build-Dep-Foo: libfoo-dev\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* a:b: c\n"),
            symbols(
                "libfoo.so.1 libfoo1 1.0\n* Build-Depends-Package: x\nlibfoo.so.2 libfoo2 2.0\n* Bad: y\n",
            ),
            symbols("libfoo.so.1 libfoo1 1.0\n* Bad: x\nlibfoo.so.1 libfoo1b 1.0\n"),
            symbols("libfoo.so.1 libfoo1 1.0\n* Bad: x\n* Worse: y\n").udeb(),
        ];
        BinaryPackageTester::new(UnknownMetaFieldInSymbolsFile, pass, fail).test_and_snapshot();
    }
}
