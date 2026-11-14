use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct FieldTooLong;

declare_rule! {
    /// A binary control field is longer than 5000 characters.
    ///
    /// Reads every Installation control field except `Build-Ids`,
    /// `Description`, `Package-List`, `Installed-Build-Depends`,
    /// `Checksums-Sha256`, and `Static-Built-Using`. The gate is the untrimmed
    /// length; the reported length is that value trimmed. Extra is the field
    /// name and `(N chars > 5000)`. Udebs are checked too.
    FieldTooLong,
    code = "LN0228",
    tag = "field-too-long",
    default_selected = true,
    experimental = false,
}

const MAXIMUM_LENGTH: usize = 5000;

const ALLOWED_FIELDS: &[&str] = &[
    "Build-Ids",
    "Description",
    "Package-List",
    "Installed-Build-Depends",
    "Checksums-Sha256",
    "Static-Built-Using",
];

impl BinaryPackageRule for FieldTooLong {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            fields
                .names()
                .filter(|name| !ALLOWED_FIELDS.contains(name))
                .filter_map(|name| {
                    let untrimmed = fields.untrimmed_value(name)?;
                    if untrimmed.chars().count() <= MAXIMUM_LENGTH {
                        return None;
                    }
                    let length = untrimmed.trim().chars().count();
                    Some(format!("{name} ({length} chars > {MAXIMUM_LENGTH})"))
                })
                .collect()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let at_limit = "a".repeat(MAXIMUM_LENGTH);
        let over = "a".repeat(MAXIMUM_LENGTH + 1);
        let padded = format!(" {} ", "a".repeat(MAXIMUM_LENGTH - 1));
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            fields(&[("Depends", &at_limit)]),
            fields(&[("Description", &over)]),
            fields(&[("Static-Built-Using", &over)]),
            fields(&[("Build-Ids", &over)]),
            fields(&[("Package-List", &over)]),
            fields(&[("Installed-Build-Depends", &over)]),
            fields(&[("Checksums-Sha256", &over)]),
            fields(&[("Depends", &"é".repeat(MAXIMUM_LENGTH))]),
        ];
        let fail = vec![
            fields(&[("Depends", &over)]),
            fields(&[("Depends", &"é".repeat(MAXIMUM_LENGTH + 1))]),
            fields(&[("Provides", &"b".repeat(6000))]),
            fields(&[("Depends", &padded)]),
            fields(&[
                ("Depends", &"a".repeat(MAXIMUM_LENGTH + 1)),
                ("Suggests", &"b".repeat(MAXIMUM_LENGTH + 2)),
            ]),
            fields(&[("Homepage", &over)]).udeb(),
        ];
        BinaryPackageTester::new(FieldTooLong, pass, fail).test_and_snapshot();
    }
}
