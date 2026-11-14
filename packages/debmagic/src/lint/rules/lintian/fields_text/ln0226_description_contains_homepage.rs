use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionContainsHomepage;

declare_rule! {
    /// The extended description still has an old `Homepage:` pseudo-field.
    ///
    /// Reads Installation control. A line matches `Homepage:` plus an `http`
    /// or `https` URL, optional `<`, any case, after leading space. Blank
    /// ` .` separators are skipped and still counted. Extra is `line N`.
    /// The synopsis is not checked. Udebs are checked too.
    DescriptionContainsHomepage,
    code = "LN0226",
    tag = "description-contains-homepage",
    default_selected = true,
    experimental = false,
}

static HOMEPAGE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*Homepage: <?https?://").expect("valid regex"));

impl BinaryPackageRule for DescriptionContainsHomepage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if HOMEPAGE_LINE.is_match(line) {
                ctx.diagnostic(Diagnostic::warn(format!("line {position}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("synopsis\n more text about the package\n"),
            description("Homepage: http://example.com/\n the synopsis is not an extended line\n"),
            description("synopsis\n See the Homepage: http://example.com/ later\n"),
            description("synopsis\n Homepage: ftp://example.com/\n"),
        ];
        let fail = vec![
            description(
                "some Lintian test package\n some Lintian test package\n .\n \
                 Some more stuff. (dummy)\n .\n Homepage: <http://lintian.debian.org/>\n",
            ),
            description("synopsis\n Homepage: http://example.com/\n"),
            description("synopsis\n  homepage: https://example.com/\n"),
            description(
                "synopsis\n text\n .\n Homepage: http://a.example/\n Homepage: <https://b.example/>\n",
            ),
            description("synopsis\n Homepage: http://example.com/\n").udeb(),
        ];
        BinaryPackageTester::new(DescriptionContainsHomepage, pass, fail).test_and_snapshot();
    }
}
