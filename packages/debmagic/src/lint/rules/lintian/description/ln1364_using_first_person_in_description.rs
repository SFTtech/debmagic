use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct UsingFirstPersonInDescription;

declare_rule! {
    /// An extended description line uses a first-person pronoun such as `I`,
    /// `my`, `we` or `us`.
    ///
    /// Each extended line (not a ` .` separator) is searched for the words
    /// `I`, `my`, `our`, `ours`, `mine`, `myself`, `me`, `us` and `we` (any
    /// capitalization of `my`, `our`, `ours` and `we`) as whole words. Each
    /// distinct word is reported once per line, with extra `line N: <word>`
    /// counting from 1 at the first extended line. As lintian does, a found
    /// word is removed by its first literal occurrence before the search
    /// goes on. Location is empty. Udebs are checked too.
    UsingFirstPersonInDescription,
    code = "LN1364",
    tag = "using-first-person-in-description",
    default_selected = false,
    experimental = false,
}

static FIRST_PERSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\s)(I|[Mm]y|[Oo]urs?|mine|myself|me|us|[Ww]e)(?:$|\s)").expect("valid regex")
});

impl BinaryPackageRule for UsingFirstPersonInDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            let mut seen = HashSet::new();
            let mut rest = line.to_string();
            while let Some(captures) = FIRST_PERSON.captures(&rest) {
                let word = captures[1].to_string();
                rest = rest.replacen(&word, "", 1);
                if seen.insert(word.clone()) {
                    ctx.diagnostic(
                        Diagnostic::warn(format!("line {position}: {word}"))
                            .with_severity(Severity::Info),
                    );
                }
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
            description("synopsis\n Tools for the job."),
            // Substrings are not words.
            description("synopsis\n Imagine usage, mything and Weather."),
            // The synopsis is not an extended line.
            description("I am the synopsis\n Tools."),
        ];
        let fail = vec![
            description("synopsis\n I am a tool."),
            description("synopsis\n Tools.\n We and us, we.\n .\n Mine too."),
            description("synopsis\n myself and my tools.").udeb(),
        ];
        BinaryPackageTester::new(UsingFirstPersonInDescription, pass, fail).test_and_snapshot();
    }
}
