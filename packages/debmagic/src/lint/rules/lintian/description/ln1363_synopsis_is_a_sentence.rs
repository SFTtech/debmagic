use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct SynopsisIsASentence;

declare_rule! {
    /// The synopsis ends a sentence, or starts another one.
    ///
    /// The synopsis is read as lintian does: the last full stop that ends the
    /// synopsis or is followed by whitespace. That full stop is not reported
    /// when the text before it ends in ` etc.`, ` e.g.`, ` eg.`, or a `...`
    /// ellipsis that is not preceded by another full stop. Extra is the
    /// synopsis in double quotes. Location is empty. Udebs are checked too.
    SynopsisIsASentence,
    code = "LN1363",
    tag = "synopsis-is-a-sentence",
    default_selected = false,
    experimental = false,
}

static EXCUSED_ENDINGS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"\s+etc\.$").expect("valid regex"),
        Regex::new(r"\s+e\.?g\.$").expect("valid regex"),
        Regex::new(r"(?:^|[^.])\.\.\.$").expect("valid regex"),
    ]
});

/// `(.*\.)(?:\s*$|\s+\S+)`: the byte index of the last full stop that ends the
/// synopsis or is followed by whitespace.
fn sentence_end(synopsis: &str) -> Option<usize> {
    synopsis
        .char_indices()
        .rev()
        .filter(|(_, c)| *c == '.')
        .map(|(index, _)| index)
        .find(|&index| {
            let rest = &synopsis[index + 1..];
            rest.chars().all(char::is_whitespace) || rest.starts_with(char::is_whitespace)
        })
}

impl BinaryPackageRule for SynopsisIsASentence {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if synopsis.trim().is_empty() {
            return;
        }
        let Some(end) = sentence_end(synopsis) else {
            return;
        };
        let before = &synopsis[..=end];
        if EXCUSED_ENDINGS.iter().any(|regex| regex.is_match(before)) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(format!("\"{synopsis}\"")).with_severity(Severity::Info));
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
            description("A tool\n Long text."),
            description("A tool for e.g. users\n Long text."),
            description("Tools, etc.\n Long text.").udeb(),
            description("Tool.file\n Long text."),
            description("Tools for: a.b\n Long text."),
        ];
        let fail = vec![
            description("This starts a sentence. Then another.\n Long text."),
            description("This ends with a full-stop.\n Long text."),
            description("This ends with a full-stop.  "),
            description("This contains a 2-ellipsis .. and other words\n Long text."),
            description("Tools.").udeb(),
        ];
        BinaryPackageTester::new(SynopsisIsASentence, pass, fail).test_and_snapshot();
    }
}
