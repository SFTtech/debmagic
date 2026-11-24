use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

pub struct DebianRulesMissingRecommendedTarget;

declare_rule! {
    /// `debian/rules` does not provide all recommended targets. Lintian 2.144.0
    /// never reports this.
    ///
    /// The lintian Check `debian/rules` lists its required targets
    /// (`debian-rules-missing-required-target`) and keeps no list of
    /// recommended ones: the Tag's explanation says "There are currently no
    /// recommended targets. Please contact the Lintian maintainers". No code
    /// path emits the Tag, which appears only in its `.tag` file and in the
    /// recipe `checks/debian/rules/rules-variable-targets`. This Rule follows
    /// that behaviour and reports nothing for the Source tree or Source
    /// package. Extra would be empty and the Location the `debian/rules` file.
    DebianRulesMissingRecommendedTarget,
    code = "LN1769",
    tag = "debian-rules-missing-recommended-target",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianRulesMissingRecommendedTarget {
    fn run(&self, _ctx: &mut SourceContext<'_>) {}
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn rules(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/rules", TestFile::new(content))]
    }

    #[test]
    fn test() {
        // Lintian never emits this Tag, so every debian/rules passes: one with
        // all the required targets, one with only `binary`, one that is not
        // UTF-8, a symlink to a real rules file, and a dangling symlink.
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            rules(
                "#!/usr/bin/make -f\nbuild build-arch build-indep:\nclean:\nbinary binary-arch binary-indep:\n",
            ),
            rules("#!/usr/bin/make -f\nbinary:\n"),
            vec![(
                "debian/rules",
                TestFile::from_bytes(b"#!/usr/bin/make -f\n\xff\n".to_vec()),
            )],
            vec![
                ("debian/rules", TestFile::symlink("real")),
                (
                    "debian/real",
                    TestFile::new("#!/usr/bin/make -f\nbinary:\n"),
                ),
            ],
            vec![("debian/rules", TestFile::symlink("missing"))],
        ];
        SourceTreeTester::new(
            DebianRulesMissingRecommendedTarget,
            pass,
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "#!/usr/bin/make -f\nbinary:\n",
        ))];
        SourcePackageTester::new(
            DebianRulesMissingRecommendedTarget,
            pass,
            Vec::<SourcePackageCase>::new(),
        )
        .test();
    }
}
