use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::{Description, description};

pub struct DescriptionPossiblyContainsHomepage;

declare_rule! {
    /// The extended Description probably gives the homepage, although the
    /// package has no `Homepage` field.
    ///
    /// Lintian's `fields/description` Check, on Installation control. Reported
    /// when the Package has no `Homepage` field, no extended line starts with
    /// `Homepage: http` (that is `description-contains-homepage`, reported by
    /// its own Rule), and either the extended description has a word such as
    /// `homepage`, `website`, `upstream` or `more info` next to an `http(s)`
    /// URL, or it ends with one. Extra is the first such URL, as written.
    /// Location is empty. Udebs are checked too.
    DescriptionPossiblyContainsHomepage,
    code = "LN1544",
    tag = "description-possibly-contains-homepage",
    default_selected = false,
    experimental = false,
}

static HOMEPAGE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*Homepage: <?https?://").expect("valid regex"));

static HOMEPAGE_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)homepage|webpage|website|url|upstream|web\s+site|home\s+page|further\s+information|more\s+info|official\s+site|project\s+home",
    )
    .expect("valid regex")
});

static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(https?://[a-z0-9][^>\s]+)").expect("valid regex"));

static URL_AT_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(https?://[a-z0-9][^>\s]+)>?\.?\s*\z").expect("valid regex")
});

impl BinaryPackageRule for DescriptionPossiblyContainsHomepage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.installation_control().fields().declares("Homepage") {
            return;
        }
        let Some(description) = description(ctx) else {
            return;
        };
        if let Some(url) = possible_homepage(&description) {
            ctx.diagnostic(Diagnostic::warn(url).with_severity(Severity::Info));
        }
    }
}

/// The URL that lintian reports, if the extended description has one. A
/// `Homepage: http` line anywhere in it suppresses the report.
fn possible_homepage(description: &Description) -> Option<String> {
    if description
        .numbered_lines()
        .any(|(_, line)| HOMEPAGE_LINE.is_match(line))
    {
        return None;
    }
    let extended = description.lines.join("\n");
    if HOMEPAGE_WORD.is_match(&extended)
        && let Some(captures) = URL.captures(&extended)
    {
        return Some(captures[1].to_string());
    }
    URL_AT_END
        .captures(&extended)
        .map(|captures| captures[1].to_string())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn binary() {
        let pass = vec![
            description("Example\n Some text about the tool."),
            description("Example\n See https://example.org/tool for details."),
            description("Example\n Homepage: https://example.org/tool"),
            description(
                "Example\n Homepage: <https://example.org/tool>\n Upstream: https://example.org/",
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field(
                    "Description",
                    "Example\n More info at https://example.org/tool",
                )
                .field("Homepage", "https://example.org/tool"),
        ];
        let fail = vec![
            description("Example\n More info at https://example.org/tool"),
            description("Example\n Maintained upstream, see https://example.org/tool."),
            description("Example\n Source code\n https://example.org/tool"),
            description("Example\n Source code\n https://example.org/tool").udeb(),
        ];
        BinaryPackageTester::new(DescriptionPossiblyContainsHomepage, pass, fail)
            .test_and_snapshot();
    }
}
