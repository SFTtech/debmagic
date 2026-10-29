use super::rule::{BinaryPackageRule, RuleAccess, SourceRule};
use super::rules::{
    ChangelogFileNotCompressed, CustomCompressionInDebianRules,
    DebianChangelogFileMissingOrWrongName, DebianRulesIgnoresMakeCleanError,
    DebianRulesMissingRequiredTarget, DebmagicDummyTrigger, DmUploadAllowedIsObsolete,
    EmptyBinaryPackage, FileInUnusualDir, LatestDebianChangelogEntryWithoutNewDate,
    MaintainerScriptEmpty, NationalEncoding, NoChangelog, NoCopyrightFile, NoDep5Copyright,
    NoHumanMaintainers, NonStandardToplevelDir, PackageContainsUnsafeSymlink,
    PackageRelationWithSelf, PackageUsesDeprecatedDebhelperCompatVersion, RequiredField,
    SourceContainsDebianSubstvars, SourceContainsPrebuiltWindowsBinary, SuperfluousFilePattern,
    SyntaxErrorInDebianChangelog, UnstrippedBinaryOrObject, UsesDebhelperCompatFile,
    VcsObsoleteInDebianInfrastructure,
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
        &DebianChangelogFileMissingOrWrongName,
        &MaintainerScriptEmpty,
        &SourceContainsDebianSubstvars,
        &CustomCompressionInDebianRules,
        &DebianRulesIgnoresMakeCleanError,
        &NationalEncoding,
        &PackageUsesDeprecatedDebhelperCompatVersion,
        &UsesDebhelperCompatFile,
        &NoHumanMaintainers,
        &DmUploadAllowedIsObsolete,
        &VcsObsoleteInDebianInfrastructure,
        &ChangelogFileNotCompressed,
        &NonStandardToplevelDir,
        &LatestDebianChangelogEntryWithoutNewDate,
        &PackageRelationWithSelf,
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
        &SourceContainsDebianSubstvars,
        &CustomCompressionInDebianRules,
        &DebianRulesIgnoresMakeCleanError,
        &NationalEncoding,
        &PackageUsesDeprecatedDebhelperCompatVersion,
        &UsesDebhelperCompatFile,
        &NoHumanMaintainers,
        &DmUploadAllowedIsObsolete,
        &VcsObsoleteInDebianInfrastructure,
        &LatestDebianChangelogEntryWithoutNewDate,
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
        &DebianChangelogFileMissingOrWrongName,
        &MaintainerScriptEmpty,
        &NationalEncoding,
        &ChangelogFileNotCompressed,
        &NonStandardToplevelDir,
        &PackageRelationWithSelf,
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
                SourceContainsDebianSubstvars::CODE,
                CustomCompressionInDebianRules::CODE,
                DebianRulesIgnoresMakeCleanError::CODE,
                NationalEncoding::CODE,
                PackageUsesDeprecatedDebhelperCompatVersion::CODE,
                UsesDebhelperCompatFile::CODE,
                NoHumanMaintainers::CODE,
                DmUploadAllowedIsObsolete::CODE,
                VcsObsoleteInDebianInfrastructure::CODE,
                LatestDebianChangelogEntryWithoutNewDate::CODE,
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
                DebianChangelogFileMissingOrWrongName::CODE,
                MaintainerScriptEmpty::CODE,
                NationalEncoding::CODE,
                ChangelogFileNotCompressed::CODE,
                NonStandardToplevelDir::CODE,
                PackageRelationWithSelf::CODE,
            ]
        );
    }
}
