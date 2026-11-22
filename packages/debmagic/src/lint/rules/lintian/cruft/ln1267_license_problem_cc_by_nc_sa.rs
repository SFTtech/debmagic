use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, BadLicense, TextFile};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemCcByNcSa;

declare_rule! {
    /// A Patched file is copyrighted under the non-free Creative Commons
    /// Non-Commercial Share-Alike (CC-NC-SA) license.
    ///
    /// Each regular text file of the Patched files (paths under `.pc/`
    /// skipped) is matched as lintian's `cruft` Check does: a keyword, a
    /// sentence and the license regex of the cleaned text. Skipped in a
    /// non-free Source package (see `license_text::is_non_free`). A file
    /// whose first `<link rel="copyright">` points at a
    /// `creativecommons.org/licenses/by-nc-sa/` URL is reported as well,
    /// in any Source package. Each finding has no extra and the path as
    /// the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks.
    LicenseProblemCcByNcSa,
    code = "LN1267",
    tag = "license-problem-cc-by-nc-sa",
    default_selected = true,
    experimental = false,
}

static CC_BY_NC_SA: LazyLock<BadLicense> = LazyLock::new(|| {
    BadLicense::new(
        &["license", "by-nc-sa", "creativecommons.org"],
        &[
            "://creativecommons.org/licenses/by-nc-sa",
            "under attribution-noncommercial",
        ],
        Some(
            r"(?:license [ ] rdf:[^=:]+=[ ]* (?:ht|f)tps?://(?:[^/.]\.)??creativecommons\.org/licenses/by-nc-sa/\d+(?:\.\d+)?(?:/[[:alpha:]]+)?/? [ ]* >|available [ ] under [ ] attribution-noncommercial)",
        ),
    )
});

static COPYRIGHT_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<link\s+rel="copyright"\s+href="([^"]+)"\s*/?\s*>"#).expect("valid regex")
});

impl SourceRule for LicenseProblemCcByNcSa {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let non_free = license_text::is_non_free(ctx);
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<_> = license_text::text_files(files)
            .into_iter()
            .filter(|file| (!non_free && CC_BY_NC_SA.matches(file)) || links_to_cc(file))
            .map(|file| file.path)
            .collect();
        for path in flagged {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The first `rel="copyright"` link of the file names a CC-NC-SA URL.
fn links_to_cc(file: &TextFile) -> bool {
    if !file.lowercase.contains(r#"rel="copyright""#) {
        return false;
    }
    COPYRIGHT_LINK
        .captures(&file.lowercase)
        .and_then(|captures| captures.get(1))
        .is_some_and(|href| {
            let href = href.as_str();
            href.starts_with("http://creativecommons.org/licenses/by-nc-sa/")
                || href.starts_with("https://creativecommons.org/licenses/by-nc-sa/")
        })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "int main() { return 0; }\n")]),
            TestCase::from(vec![(
                "src/main.html",
                "<link rel=\"copyright\" href=\"https://example.org/licenses/\">\n",
            )]),
        ];
        let fail = vec![
            TestCase::from(vec![(
                "src/main.c",
                " * License: available under Attribution-NonCommercial 3.0\n",
            )]),
            TestCase::from(vec![(
                "src/main.html",
                "<link rel=\"copyright\" href=\"http://creativecommons.org/licenses/by-nc-sa/3.0/\">\n",
            )]),
        ];
        SourceTreeTester::new(LicenseProblemCcByNcSa, pass, fail).test_and_snapshot();
    }
}
