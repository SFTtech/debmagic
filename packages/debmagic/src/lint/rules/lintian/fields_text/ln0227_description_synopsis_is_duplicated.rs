use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::{description, is_blank_separator};

pub struct DescriptionSynopsisIsDuplicated;

declare_rule! {
    /// The first extended-description line repeats the synopsis.
    ///
    /// Reads Installation control. Compared case-insensitively, then again
    /// with non-alphanumerics removed, so a leading space still matches.
    /// Only position 1 is checked; a leading ` .` skips the tag. Extra is
    /// `line 1`. Udebs are checked too.
    DescriptionSynopsisIsDuplicated,
    code = "LN0227",
    tag = "description-synopsis-is-duplicated",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionSynopsisIsDuplicated {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let Some(line) = description.lines.first() else {
            return;
        };
        if is_blank_separator(line) {
            return;
        }
        if synopsis_repeated(&description.synopsis, line) {
            ctx.diagnostic(Diagnostic::error("line 1"));
        }
    }
}

fn synopsis_repeated(synopsis: &str, line: &str) -> bool {
    let synopsis = synopsis.to_lowercase();
    let line = line.to_lowercase();
    if line == synopsis {
        return true;
    }
    ascii_alnum(&line) == ascii_alnum(&synopsis)
}

fn ascii_alnum(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
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
            description("synopsis only"),
            description(
                "some Lintian test package\n a different first line\n some Lintian test package\n",
            ),
            description("synopsis\n .\n synopsis\n"),
            description("foo bar\n foo baz\n"),
        ];
        let fail = vec![
            description("some Lintian test package\n some Lintian test package\n .\n more\n"),
            description("foo-bar\n foo bar.\n"),
            description("Foo\n foo\n"),
            description("---\n !!!\n"),
            description("synopsis\n synopsis\n").udeb(),
        ];
        BinaryPackageTester::new(DescriptionSynopsisIsDuplicated, pass, fail).test_and_snapshot();
    }
}
