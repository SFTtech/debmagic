use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct ExtendedDescriptionLineTooLong;

declare_rule! {
    /// A line of the extended description is longer than 80 characters.
    ///
    /// The line is measured with its leading space; lines made of only ` .`
    /// are skipped but still counted. Extra is `line N` with N counting from 1
    /// at the first line after the synopsis. Location is empty. Udebs are
    /// checked too.
    ExtendedDescriptionLineTooLong,
    code = "LN0056",
    tag = "extended-description-line-too-long",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExtendedDescriptionLineTooLong {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if line.chars().count() > 80 {
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
            description(&format!("synopsis\n {}", "x".repeat(79))),
            description(&format!("{}\n short", "x".repeat(200))),
        ];
        let fail = vec![
            description(&format!("synopsis\n {}", "x".repeat(80))),
            description(&format!(
                "synopsis\n short\n .\n {}\n {}",
                "x".repeat(90),
                "y".repeat(80)
            )),
            description(&format!("synopsis\n {}", "x".repeat(80))).udeb(),
        ];
        BinaryPackageTester::new(ExtendedDescriptionLineTooLong, pass, fail).test_and_snapshot();
    }
}
