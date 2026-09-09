use super::rule::{BinaryPackageRule, RuleAccess, SourcePackageRule, SourceTreeRule};
use super::rules::{
    DebianRulesMissingRequiredTarget, DebmagicDummyTrigger, RequiredField,
    SyntaxErrorInDebianChangelog,
};

pub(crate) fn all_rules() -> &'static [&'static dyn RuleAccess] {
    &[
        &DebmagicDummyTrigger,
        &RequiredField,
        &SyntaxErrorInDebianChangelog,
        &DebianRulesMissingRequiredTarget,
    ]
}

pub(crate) fn source_tree_rules() -> &'static [&'static dyn SourceTreeRule] {
    &[
        &DebmagicDummyTrigger,
        &RequiredField,
        &SyntaxErrorInDebianChangelog,
        &DebianRulesMissingRequiredTarget,
    ]
}

pub(crate) fn binary_package_rules() -> &'static [&'static dyn BinaryPackageRule] {
    &[&RequiredField]
}

pub(crate) fn source_package_rules() -> &'static [&'static dyn SourcePackageRule] {
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::rule::RuleMeta;

    #[test]
    fn source_package_has_no_rules_yet() {
        assert!(source_package_rules().is_empty());
    }

    #[test]
    fn binary_package_rules_are_required_field() {
        let codes: Vec<_> = binary_package_rules()
            .iter()
            .map(|rule| rule.code())
            .collect();
        assert_eq!(codes, vec![RequiredField::CODE]);
    }
}
