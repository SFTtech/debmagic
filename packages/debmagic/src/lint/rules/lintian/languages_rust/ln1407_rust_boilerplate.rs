use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct RustBoilerplate;

declare_rule! {
    /// An installable's extended description still starts with the text that
    /// debcargo's template writes: `This package contains the following
    /// binaries built from the Rust crate`, matched without regard to case.
    ///
    /// The synopsis, the first line of `Description`, is dropped; each line's
    /// one leading space or tab is removed, as lintian's `text` does. Extra is
    /// the installable's name. Applies to the Source tree and the Source
    /// package.
    RustBoilerplate,
    code = "LN1407",
    tag = "rust-boilerplate",
    default_selected = true,
    experimental = false,
}

const TEMPLATE: &str = "This package contains the following binaries built from the Rust crate";

impl SourceRule for RustBoilerplate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let names: Vec<String> = control
            .installables()
            .iter()
            .filter_map(|installable| {
                let description = installable.fields().untrimmed_value("Description")?;
                is_template(description).then(|| installable.name().to_string())
            })
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

/// lintian's `text` (one leading space or tab off each line, a lone `.` as an
/// empty line), then the synopsis line dropped, then the template match.
fn is_template(untrimmed: &str) -> bool {
    let text: String = untrimmed
        .split('\n')
        .map(|line| {
            let line = line
                .strip_prefix(' ')
                .or_else(|| line.strip_prefix('\t'))
                .unwrap_or(line);
            if line == "." { "" } else { line }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Some((_, extended)) = text.split_once('\n') else {
        return false;
    };
    extended
        .get(..TEMPLATE.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(TEMPLATE))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(description: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nBuild-Depends: debhelper-compat (= 14)\n\nPackage: example\nArchitecture: any\nDescription: {description}\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control("Command-line tool\n This is a hand-written description.\n"),
            // Only the synopsis: nothing after it.
            control(
                "This package contains the following binaries built from the Rust crate hyperfine",
            ),
            // The template is not at the start of the extended description.
            control(
                "Tool\n Some text.\n This package contains the following binaries built from the Rust crate x\n",
            ),
            // A `.` line before the template leaves an empty first line.
            control(
                "Tool\n .\n This package contains the following binaries built from the Rust crate x\n",
            ),
        ];
        let fail = vec![
            control(
                "Command-line tool\n This package contains the following binaries built from the Rust crate hyperfine\n",
            ),
            control(
                "Debcargo's default\n This package contains the following binaries built from the Rust crate \"hyperfine\": - hyperfine\n",
            ),
            // Case does not matter.
            control(
                "Tool\n this PACKAGE contains the following binaries built from the Rust crate x\n",
            ),
        ];
        SourceTreeTester::new(RustBoilerplate, pass, fail).test_and_snapshot();
    }
}
