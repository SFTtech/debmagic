use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UsesImplicitAwaitTrigger;

declare_rule! {
    /// A triggers directive is an implicit await trigger.
    ///
    /// The control member is the `triggers` file. Blank lines and comments are
    /// skipped. A directive needs a type and a name. `interest` and `activate`
    /// are the implicit-await types in lintian's `triggers/trigger-types`;
    /// `-await` and `-noawait` are not. Unknown types are not this Tag. Extra
    /// is the trigger type. Udebs are checked too.
    UsesImplicitAwaitTrigger,
    code = "LN0355",
    tag = "uses-implicit-await-trigger",
    default_selected = true,
    experimental = false,
}

const IMPLICIT_AWAIT: &[&str] = &["activate", "interest"];

impl BinaryPackageRule for UsesImplicitAwaitTrigger {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = control_text(ctx, "triggers") else {
            return;
        };
        for trigger_type in implicit_await_types(&text) {
            ctx.diagnostic(Diagnostic::warn(trigger_type));
        }
    }
}

fn control_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get(name)?;
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

fn implicit_await_types(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let trigger_type = parts.next()?;
            parts.next()?;
            IMPLICIT_AWAIT
                .contains(&trigger_type)
                .then(|| trigger_type.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn triggers(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("triggers", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            triggers(""),
            triggers(
                "# comment\n\ninterest-noawait /usr/share/foo\ninterest-await /usr/share/bar\n",
            ),
            triggers("activate-noawait foo\nactivate-await bar\n"),
            triggers("interest\nactivate\n"),
            triggers("random-trigger /bar\n"),
        ];
        let fail = vec![
            triggers(
                "# implicit\ninterest /usr/share/lintian/baz\ninterest-await /usr/share/other\n",
            ),
            triggers("activate foo\n\ninterest bar\n"),
            triggers("interest /usr/share/foo\n").udeb(),
        ];
        BinaryPackageTester::new(UsesImplicitAwaitTrigger, pass, fail).test_and_snapshot();
    }
}
