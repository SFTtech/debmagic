use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_RELATION_FIELDS;

pub struct BadProvidedPackageName;

declare_rule! {
    /// A relationship names a package that is not a Debian package name.
    ///
    /// Every alternative of `Depends`, `Pre-Depends`, `Recommends`, `Suggests`,
    /// `Conflicts`, `Provides`, `Enhances`, `Replaces`, and `Breaks`. The name,
    /// without an `:arch` qualifier, must match `[a-z0-9][-+.a-z0-9]+`: at
    /// least two characters, starting with a lowercase letter or digit. Extra
    /// is that name. An empty parse is skipped. Applies to debs and udebs.
    BadProvidedPackageName,
    code = "LN0267",
    tag = "bad-provided-package-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadProvidedPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    BINARY_RELATION_FIELDS
        .iter()
        .filter_map(|field| fields.relation(field))
        .flat_map(|relation| {
            relation
                .alternatives()
                .filter(|alternative| bad_package_name(alternative.package()))
                .map(|alternative| alternative.package().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// lintian's `/^[a-z0-9][-+\.a-z0-9]+$/`, inverted. Empty is not a name.
fn bad_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_digit() && !first.is_ascii_lowercase() {
        return true;
    }
    let mut rest = 0usize;
    for ch in chars {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.')) {
            return true;
        }
        rest += 1;
    }
    rest < 1
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
        let pass = vec![
            fields(&[]),
            fields(&[("Provides", "libc6, libstdc++6, a1, 123456789, c++")]),
            fields(&[("Depends", "libc6:any")]),
            fields(&[("Provides", "valid-versioned-provides (= 1.0)")]),
            fields(&[("Provides", "arch-specific [i386]")]),
            fields(&[("Depends", "ok, , also-ok")]),
        ];
        let fail = vec![
            fields(&[("Provides", "a, iNvAlId-NaMe")]),
            fields(&[("Depends", "BadName, _foo, -foo")]),
            fields(&[("Conflicts", "Foo:any")]),
            fields(&[("Replaces", "A")]).udeb(),
        ];
        BinaryPackageTester::new(BadProvidedPackageName, pass, fail).test_and_snapshot();
    }
}
