use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct EmbeddedFeedparserLibrary;

declare_rule! {
    /// An embedded copy of the Universal Feed Parser.
    ///
    /// A regular file or hardlink whose path ends in `/feedparser.py` and whose
    /// bytes contain `Universal feed parser`. The source package `feedparser`
    /// is exempt. Extra is empty. The path is the Location. Udebs are checked
    /// too.
    EmbeddedFeedparserLibrary,
    code = "LN0479",
    tag = "embedded-feedparser-library",
    default_selected = true,
    experimental = false,
}

const MARKER: &[u8] = b"Universal feed parser";

static FEEDPARSER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/feedparser\.py$").expect("valid regex"));

impl BinaryPackageRule for EmbeddedFeedparserLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "feedparser" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if !FEEDPARSER.is_match(path) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if bytes.windows(MARKER.len()).any(|window| window == MARKER) {
                paths.push(entry.path().to_path_buf());
            }
        }
        paths.sort();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn parser(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let marker = "\"\"\"Universal feed parser\n";
        let sentence = "This is not the \"Universal feed parser\", but it will do.";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            parser("feedparser.py", marker),
            parser("files/feedparser.py", "a different library"),
            parser("files/myfeedparser.py", marker),
            parser("files/feedparser.py.bak", marker),
            parser("files/Feedparser.py", marker),
            parser("files/feedparser.py", marker).field("Source", "feedparser"),
            parser("files/feedparser.py", marker).field("Source", "feedparser (5.0)"),
            BinaryPackageCase::from(vec![(
                "files/feedparser.py".to_string(),
                TestFile::symlink("real.py"),
            )]),
        ];
        let fail = vec![
            parser("files/feedparser.py", marker),
            parser("usr/share/php/feedparser.py", sentence),
            BinaryPackageCase::from(vec![(
                "usr/lib/feedparser.py".to_string(),
                TestFile::new(marker).hardlink(),
            )]),
            parser("opt/feedparser.py", marker).udeb(),
            BinaryPackageCase::from(vec![
                (
                    "a/feedparser.py".to_string(),
                    TestFile::new("prefix Universal feed parser suffix"),
                ),
                ("b/feedparser.py".to_string(), TestFile::new(marker)),
            ]),
        ];
        BinaryPackageTester::new(EmbeddedFeedparserLibrary, pass, fail).test_and_snapshot();
    }
}
