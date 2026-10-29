use super::rule::{BinaryPackageRule, RuleAccess, SourceRule};
use super::rules::{
    DebianRulesMissingRequiredTarget, DebmagicDummyTrigger, EmptyBinaryPackage, FileInUnusualDir,
    NoChangelog, NoCopyrightFile, NoDep5Copyright, PackageContainsUnsafeSymlink, RequiredField,
    SourceContainsPrebuiltWindowsBinary, SuperfluousFilePattern, SyntaxErrorInDebianChangelog,
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
        &EmptyBinaryPackage,
        &SuperfluousFilePattern,
        &NoDep5Copyright,
        &NoCopyrightFile,
        &FileInUnusualDir,
        &NoChangelog,
    ]
}

pub(crate) fn source_rules() -> &'static [&'static dyn SourceRule] {
    &[
        &DebmagicDummyTrigger,
        &RequiredField,
        &SyntaxErrorInDebianChangelog,
        &DebianRulesMissingRequiredTarget,
        &SourceContainsPrebuiltWindowsBinary,
        &SuperfluousFilePattern,
        &NoDep5Copyright,
    ]
}

pub(crate) fn binary_package_rules() -> &'static [&'static dyn BinaryPackageRule] {
    &[
        &RequiredField,
        &UnstrippedBinaryOrObject,
        &PackageContainsUnsafeSymlink,
        &EmptyBinaryPackage,
        &NoCopyrightFile,
        &FileInUnusualDir,
        &NoChangelog,
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
                SuperfluousFilePattern::CODE,
                NoDep5Copyright::CODE,
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
                EmptyBinaryPackage::CODE,
                NoCopyrightFile::CODE,
                FileInUnusualDir::CODE,
                NoChangelog::CODE,
            ]
        );
    }
}
