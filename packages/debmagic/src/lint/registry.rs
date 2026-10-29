use super::rule::{BinaryPackageRule, RuleAccess, SourceRule};
use super::rules::{
    DebianRulesMissingRequiredTarget, DebmagicDummyTrigger, PackageContainsUnsafeSymlink,
    RequiredField, SourceContainsPrebuiltWindowsBinary, SyntaxErrorInDebianChangelog,
    UnstrippedBinaryOrObject,
};

pub(crate) fn all_rules() -> &'static [&'static dyn RuleAccess] {
    &[
        &DebmagicDummyTrigger,
        &RequiredField,
        &SyntaxErrorInDebianChangelog,
        &DebianRulesMissingRequiredTarget,
        &SourceContainsPrebuiltWindowsBinary,
        &UnstrippedBinaryOrObject,
        &PackageContainsUnsafeSymlink,
    ]
}

pub(crate) fn source_rules() -> &'static [&'static dyn SourceRule] {
    &[
        &DebmagicDummyTrigger,
        &RequiredField,
        &SyntaxErrorInDebianChangelog,
        &DebianRulesMissingRequiredTarget,
        &SourceContainsPrebuiltWindowsBinary,
    ]
}

pub(crate) fn binary_package_rules() -> &'static [&'static dyn BinaryPackageRule] {
    &[
        &RequiredField,
        &UnstrippedBinaryOrObject,
        &PackageContainsUnsafeSymlink,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::rule::RuleMeta;

    #[test]
    fn source_rules_cover_catalogued_source_tags() {
        let codes: Vec<_> = source_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec![
                DebmagicDummyTrigger::CODE,
                RequiredField::CODE,
                SyntaxErrorInDebianChangelog::CODE,
                DebianRulesMissingRequiredTarget::CODE,
                SourceContainsPrebuiltWindowsBinary::CODE,
            ]
        );
    }

    #[test]
    fn binary_package_rules_cover_catalogued_binary_tags() {
        let codes: Vec<_> = binary_package_rules()
            .iter()
            .map(|rule| rule.code())
            .collect();
        assert_eq!(
            codes,
            vec![
                RequiredField::CODE,
                UnstrippedBinaryOrObject::CODE,
                PackageContainsUnsafeSymlink::CODE,
            ]
        );
    }
}
