use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct RubyInterpreterIsDeprecated;

declare_rule! {
    /// A deb's `Depends` names the `ruby-interpreter` virtual package, which
    /// Ruby interpreter packages stopped providing with ruby2.3.
    ///
    /// `Depends` is split on commas, and each element that contains the word
    /// `ruby-interpreter` (`\b` on both sides, as lintian's pattern) is reported.
    /// Extra is the whole element, trimmed, so `ruby2.7 | ruby-interpreter`
    /// reports that text. Only `Depends` is read, not `Pre-Depends`. Udebs are
    /// skipped.
    RubyInterpreterIsDeprecated,
    code = "LN1120",
    tag = "ruby-interpreter-is-deprecated",
    default_selected = true,
    experimental = false,
}

static RUBY_INTERPRETER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bruby-interpreter\b").expect("valid regex"));

impl BinaryPackageRule for RubyInterpreterIsDeprecated {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let depends = ctx
            .installation_control()
            .fields()
            .unfolded_value("Depends")
            .unwrap_or_default();
        let reports: Vec<String> = depends
            .split(',')
            .map(str::trim)
            .filter(|prerequisite| RUBY_INTERPRETER.is_match(prerequisite))
            .map(str::to_string)
            .collect();
        for prerequisite in reports {
            ctx.diagnostic(Diagnostic::warn(prerequisite));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, &str)>::new()).field("Depends", "ruby, libc6"),
            BinaryPackageCase::from(Vec::<(&str, &str)>::new())
                .field("Recommends", "ruby-interpreter"),
            BinaryPackageCase::from(Vec::<(&str, &str)>::new())
                .field("Depends", "libruby-foo, ruby-interpreters-x"),
            BinaryPackageCase::from(Vec::<(&str, &str)>::new())
                .field("Depends", "ruby-interpreter")
                .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, &str)>::new())
                .field("Depends", "ruby-interpreter"),
            BinaryPackageCase::from(Vec::<(&str, &str)>::new()).field(
                "Depends",
                "libc6, ruby-interpreter (>= 1.9), ruby2.7 | ruby-interpreter",
            ),
        ];
        BinaryPackageTester::new(RubyInterpreterIsDeprecated, pass, fail).test_and_snapshot();
    }
}
