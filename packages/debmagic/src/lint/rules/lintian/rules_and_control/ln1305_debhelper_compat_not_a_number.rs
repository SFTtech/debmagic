use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

pub struct DebhelperCompatNotANumber;

declare_rule! {
    /// `debian/compat` declares a debhelper compatibility level that is not a
    /// number. Lintian 2.144.0 never reports this.
    ///
    /// The lintian Check `debhelper` takes the compat level from
    /// `dh_assistant active-compat-level` (`Processable::Source::CompatLevel`),
    /// which yields the numeric `declared-compat-level`. The only compat-file
    /// hint in the Check, `debhelper-compat-file-contains-multiple-levels`,
    /// concerns a later line, and no code path builds this Tag: its name
    /// appears only in its `.tag` file and in the recipe
    /// `checks/debhelper/debhelper-compat`. This Rule follows that behaviour
    /// and reports nothing for the Source tree or Source package. Extra would
    /// be empty and the Subject the source package.
    DebhelperCompatNotANumber,
    code = "LN1305",
    tag = "debhelper-compat-not-a-number",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebhelperCompatNotANumber {
    fn run(&self, _ctx: &mut SourceContext<'_>) {}
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn compat(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/compat", TestFile::new(content))]
    }

    #[test]
    fn test() {
        // Lintian never emits this Tag, so a compat level that is not a number
        // passes, as does a numeric one and a tree without debian/compat.
        let pass = vec![
            compat("13\n"),
            compat("thirteen\n"),
            compat(""),
            vec![("README", TestFile::new("no compat file"))],
        ];
        SourceTreeTester::new(
            DebhelperCompatNotANumber,
            pass,
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(compat("thirteen\n"))];
        SourcePackageTester::new(
            DebhelperCompatNotANumber,
            pass,
            Vec::<SourcePackageCase>::new(),
        )
        .test();
    }
}
