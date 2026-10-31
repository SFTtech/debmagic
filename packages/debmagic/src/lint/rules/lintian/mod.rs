mod changelog_doc;
mod debian_copyright;
mod debian_rules;
mod debian_version_substvars;
mod debian_watch;
mod description;
mod domain;
mod fields_package_relations;
mod files_hierarchy;
mod files_names;
mod files_symbolic_links;
mod files_vcs;
mod known_fields;
mod languages_python;
mod ln0001_required_field;
mod ln0002_syntax_error_in_debian_changelog;
mod ln0003_debian_rules_missing_required_target;
mod ln0004_source_contains_prebuilt_windows_binary;
mod ln0005_unstripped_binary_or_object;
mod ln0007_empty_binary_package;
mod ln0008_superfluous_file_pattern;
mod ln0011_file_in_unusual_dir;
mod ln0012_no_changelog;
mod ln0013_debian_changelog_file_missing_or_wrong_name;
mod ln0014_maintainer_script_empty;
mod ln0015_source_contains_debian_substvars;
mod ln0016_custom_compression_in_debian_rules;
mod ln0017_debian_rules_ignores_make_clean_error;
mod ln0018_national_encoding;
mod ln0019_package_uses_deprecated_debhelper_compat_version;
mod ln0020_uses_debhelper_compat_file;
mod ln0021_no_human_maintainers;
mod ln0022_dm_upload_allowed_is_obsolete;
mod ln0023_vcs_obsolete_in_debian_infrastructure;
mod ln0024_changelog_file_not_compressed;
mod ln0025_non_standard_toplevel_dir;
mod ln0026_latest_debian_changelog_entry_without_new_date;
mod ln0028_malformed_dm_upload_allowed;
mod ln0032_wrong_package_mail_alias;
mod ln0033_debian_changelog_file_is_a_symlink;
mod ln0066_source_contains_cmake_cache_file;
mod ln0106_bogus_mail_host;
mod ln0122_file_in_etc_not_marked_as_conffile;
mod ln0123_no_debconf_config;
mod ln0124_unknown_field;
mod ln0132_menu_file_in_usr_lib;
mod ln0133_bad_menu_file_name;
mod ln0135_vcs_field_uses_not_recommended_uri_format;
mod mailbox;
mod systemd;
mod tlds;

pub(crate) use debian_copyright::{
    CopyrightFileCompressed, CopyrightFileIsSymlink, CopyrightRefersToCompressedLicense,
    CopyrightRefersToDeprecatedBsdLicenseFile, CopyrightRefersToIncorrectDirectory,
    CopyrightRefersToNonexistentLicenseFile, CopyrightRefersToOldDirectory,
    CopyrightRefersToVersionlessLicenseFile, NoCopyrightFile, NoDep5Copyright,
    OldFsfAddressInCopyrightFile, SpaceInStdShortnameInDep5Copyright,
};
pub(crate) use debian_version_substvars::{
    NotBinnmuableAllDependsAny, NotBinnmuableAnyDependsAll, NotBinnmuableAnyDependsAny,
};
pub(crate) use debian_watch::{
    DebianWatchCouldVerifyDownload, DebianWatchFilePubkeyFileIsMissing,
    DebianWatchFileSpecifiesOldUpstreamVersion, DebianWatchFileSpecifiesWrongUpstreamVersion,
    DebianWatchFileUsesDeprecatedGithubredir, DebianWatchFileUsesDeprecatedSfRedirectorMethod,
    DebianWatchLacksSourceforgeRedirector, DebianWatchLineInvalid, DebianWatchNotManglingVersion,
    MissingDebianWatchFileStandard, MultipleDebianWatchFileStandards,
    ObsoleteDebianWatchFileStandard, UnknownDebianWatchFileStandard,
};
pub(crate) use description::{
    DescriptionIsPkgName, DescriptionStartsWithPackageName, DescriptionSynopsisIsEmpty,
    DescriptionSynopsisStartsWithArticle, DescriptionTooShort, ExtendedDescriptionIsEmpty,
    ExtendedDescriptionLineTooLong, SynopsisTooLong,
};
pub(crate) use fields_package_relations::{
    DependsOnObsoletePackage, DependsOnOldEmacs, PackageRelationWithSelf,
};
pub(crate) use files_hierarchy::{
    AliasedLocation, DirInUsrLocal, DirOrFileInEtcOpt, DirOrFileInHome, DirOrFileInMnt,
    DirOrFileInOpt, DirOrFileInRun, DirOrFileInSrv, DirOrFileInTmp, DirOrFileInVarLock,
    DirOrFileInVarRun, DirOrFileInVarWww, FileDirectlyInUsrShare, FileInUsrLocal, FsstndDirInUsr,
    FsstndDirInVar, NonMultiArchLibDir, NonStandardDirInUsr, NonStandardDirInVar, SubdirInUsrBin,
    UseOfCompatSymlink,
};
pub(crate) use files_names::{
    FileNameContainsWildcardCharacter, FileNameEndsInWhitespace, FileNameInPathIsNotAscii,
    HyphenFile, PackageContainsCompiledGlibSchema, PackageContainsEslintConfigFile,
    PackageContainsFileInEtcSkel, PackageContainsFileInUsrShareHal,
    PackageContainsIconCacheInGenericDir, PackageContainsNpmIgnoreFile,
    PackageContainsPythonCoverageFile, PackageContainsPythonDoctreeFile,
    PackageContainsPythonDotDirectory, PackageContainsPythonHeaderInIncorrectDirectory,
    PackageContainsPythonHypothesisExample, PackageContainsPythonTestsInGlobalNamespace,
    PackageContainsSassCacheDirectory, StarFile,
};
pub(crate) use files_symbolic_links::{
    AbsoluteSymlinkInTopLevelFolder, CompressedSymlinkWithWrongExt, LengthySymlink,
    PackageContainsUnsafeSymlink, RelativeSymlink, SymlinkContainsSpuriousSegments,
    SymlinkEndsWithSlash, SymlinkHasDoubleSlash, SymlinkHasTooManyUpSegments,
    SymlinkIsSelfRecursive,
};
pub(crate) use files_vcs::{
    PackageContainsVcsControlDir, PackageContainsVcsControlFile, SvkCommitFileInPackage,
    SvnCommitFileInPackage,
};
pub(crate) use languages_python::{
    AncientPythonVersionField, FileInUsrLibSitePython, MalformedPythonVersion,
    PackageInstallsDeprecatedPython2Path, PythonVersionCurrentIsDeprecated,
    SourcePackageEncodesPythonVersion, ThirdPartyPackageInPythonDir,
};
pub(crate) use ln0001_required_field::RequiredField;
pub(crate) use ln0002_syntax_error_in_debian_changelog::SyntaxErrorInDebianChangelog;
pub(crate) use ln0003_debian_rules_missing_required_target::DebianRulesMissingRequiredTarget;
pub(crate) use ln0004_source_contains_prebuilt_windows_binary::SourceContainsPrebuiltWindowsBinary;
pub(crate) use ln0005_unstripped_binary_or_object::UnstrippedBinaryOrObject;
pub(crate) use ln0007_empty_binary_package::EmptyBinaryPackage;
pub(crate) use ln0008_superfluous_file_pattern::SuperfluousFilePattern;
pub(crate) use ln0011_file_in_unusual_dir::FileInUnusualDir;
pub(crate) use ln0012_no_changelog::NoChangelog;
pub(crate) use ln0013_debian_changelog_file_missing_or_wrong_name::DebianChangelogFileMissingOrWrongName;
pub(crate) use ln0014_maintainer_script_empty::MaintainerScriptEmpty;
pub(crate) use ln0015_source_contains_debian_substvars::SourceContainsDebianSubstvars;
pub(crate) use ln0016_custom_compression_in_debian_rules::CustomCompressionInDebianRules;
pub(crate) use ln0017_debian_rules_ignores_make_clean_error::DebianRulesIgnoresMakeCleanError;
pub(crate) use ln0018_national_encoding::NationalEncoding;
pub(crate) use ln0019_package_uses_deprecated_debhelper_compat_version::PackageUsesDeprecatedDebhelperCompatVersion;
pub(crate) use ln0020_uses_debhelper_compat_file::UsesDebhelperCompatFile;
pub(crate) use ln0021_no_human_maintainers::NoHumanMaintainers;
pub(crate) use ln0022_dm_upload_allowed_is_obsolete::DmUploadAllowedIsObsolete;
pub(crate) use ln0023_vcs_obsolete_in_debian_infrastructure::VcsObsoleteInDebianInfrastructure;
pub(crate) use ln0024_changelog_file_not_compressed::ChangelogFileNotCompressed;
pub(crate) use ln0025_non_standard_toplevel_dir::NonStandardToplevelDir;
pub(crate) use ln0026_latest_debian_changelog_entry_without_new_date::LatestDebianChangelogEntryWithoutNewDate;
pub(crate) use ln0028_malformed_dm_upload_allowed::MalformedDmUploadAllowed;
pub(crate) use ln0032_wrong_package_mail_alias::WrongPackageMailAlias;
pub(crate) use ln0033_debian_changelog_file_is_a_symlink::DebianChangelogFileIsASymlink;
pub(crate) use ln0066_source_contains_cmake_cache_file::SourceContainsCmakeCacheFile;
pub(crate) use ln0106_bogus_mail_host::BogusMailHost;
pub(crate) use ln0122_file_in_etc_not_marked_as_conffile::FileInEtcNotMarkedAsConffile;
pub(crate) use ln0123_no_debconf_config::NoDebconfConfig;
pub(crate) use ln0124_unknown_field::UnknownField;
pub(crate) use ln0132_menu_file_in_usr_lib::MenuFileInUsrLib;
pub(crate) use ln0133_bad_menu_file_name::BadMenuFileName;
pub(crate) use ln0135_vcs_field_uses_not_recommended_uri_format::VcsFieldUsesNotRecommendedUriFormat;
pub(crate) use systemd::{
    InitDScriptDoesNotSourceInitFunctions, MissingSystemdServiceForInitDScript,
};
