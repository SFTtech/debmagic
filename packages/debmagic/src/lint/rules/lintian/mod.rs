mod apache2;
mod archive_liberty;
mod autotools;
mod binaries_perl;
mod changelog_doc;
mod conffiles;
mod debhelper_rules;
mod debian_changelog;
mod debian_copyright;
mod debian_debconf;
mod debian_patches;
mod debian_po_debconf;
mod debian_readme;
mod debian_rules;
mod debian_shlibs;
mod debian_variables;
mod debian_version_substvars;
mod debian_watch;
mod description;
mod documentation_manual;
mod domain;
mod fields_mail_address;
mod fields_package_relations;
mod fields_relations_more;
mod fields_text;
mod fields_version;
mod files_artifact;
mod files_foreign;
mod files_hierarchy;
mod files_includes;
mod files_installed;
mod files_names;
mod files_symbolic_links;
mod files_vcs;
mod games;
mod init_scripts;
mod known_fields;
mod languages_javascript;
mod languages_perl;
mod languages_python;
mod ln0001_required_field;
mod ln0002_syntax_error_in_debian_changelog;
mod ln0004_source_contains_prebuilt_windows_binary;
mod ln0005_unstripped_binary_or_object;
mod ln0007_empty_binary_package;
mod ln0008_superfluous_file_pattern;
mod ln0011_file_in_unusual_dir;
mod ln0012_no_changelog;
mod ln0013_debian_changelog_file_missing_or_wrong_name;
mod ln0014_maintainer_script_empty;
mod ln0015_source_contains_debian_substvars;
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
mod ln0183_package_contains_bts_control_dir;
mod ln0184_uses_dpkg_database_directly;
mod ln0185_obsolete_url_in_packaging;
mod mailbox;
mod mailcap;
mod md5sums;
mod menu_format;
mod menus;
mod po_debconf;
mod rules_and_control;
mod scripts_calls;
mod scripts_depends;
mod scripts_shell;
mod scripts_text;
mod systemd;
mod testsuite;
mod tlds;
mod triggers;
mod vcs;

pub(crate) use apache2::{
    Apache2ConfigurationFilesNeedConfSuffix, Apache2ReverseDependencyCallsWrapperScript,
    Apache2UnparsableDependency,
};
pub(crate) use archive_liberty::ArchiveLibertyMismatch;
pub(crate) use autotools::{AncientLibtool, AutotoolsPkgConfigMacroNotCrossCompilationSafe};
pub(crate) use binaries_perl::MissingDependencyOnPerlapi;
pub(crate) use conffiles::NonEtcFileMarkedAsConffile;
pub(crate) use debhelper_rules::{
    DebhelperBaseCompatLevelIsMissing, MaintainerScriptLacksDebhelperToken,
    TypoInDebhelperOverrideTarget,
};

pub(crate) use debian_changelog::{
    BogusMailHostInDebianChangelog, ChangelogNewsDebianMismatch, DebianChangelogHasWrongDayOfWeek,
    DebianChangelogLineTooLong, DebianNewsEntryHasStrangeDistribution, EpochChangeWithoutComment,
    EpochChangedButUpstreamVersionDidNotGoBackwards, InitialUploadClosesNoBugs,
    LatestChangelogEntryWithoutNewDate, LatestDebianChangelogEntryReusesExistingVersion,
    NoNmuInChangelog, NoQaInChangelog, OddHistoricalDebianChangelogVersion,
    QaUploadHasIncorrectVersionNumber, SourceNmuHasIncorrectVersionNumber,
    SyntaxErrorInDebianNewsFile, WrongNameForDebianNewsFile,
};
pub(crate) use debian_copyright::{
    CopyrightContainsDhMakeTodoBoilerplate, CopyrightFileCompressed,
    CopyrightFileContainsFullGplLicense, CopyrightFileIsSymlink,
    CopyrightFileLacksPointerToPerlLicense, CopyrightNotUsingCommonLicenseForApache2,
    CopyrightNotUsingCommonLicenseForGpl, CopyrightNotUsingCommonLicenseForLgpl,
    CopyrightRefersToCompressedLicense, CopyrightRefersToDeprecatedBsdLicenseFile,
    CopyrightRefersToIncorrectDirectory, CopyrightRefersToNonexistentLicenseFile,
    CopyrightRefersToOldDirectory, CopyrightRefersToVersionlessLicenseFile,
    EmptyShortLicenseInDep5Copyright, IncompleteCreativeCommonsLicense,
    LicenseProblemUndefinedLicense, MissingFieldInDep5Copyright, MissingLicenseTextInDep5Copyright,
    NamedCopyrightForSingleInstallable, NoCopyrightFile, NoDebianCopyrightInSource,
    NoDep5Copyright, OldFsfAddressInCopyrightFile, SourceShipsExcludedFile,
    SpaceInStdShortnameInDep5Copyright,
};

pub(crate) use debian_debconf::{
    LoadsObsoleteConfmodule, MakingAssumptionsAboutInterfacesInTemplates,
    MalformedPromptInTemplates, MalformedQuestionInTemplates, MalformedTitleInTemplates,
    MismatchTranslatedChoices, MissingDebconfDependencyForPreinst, NoDebconfTemplates,
    NoTemplateDescription, NoTemplateType, PossibleDebconfNoteAbuse, PostinstDoesNotLoadConfmodule,
    PostinstUsesDbInput, SelectWithBooleanChoices, TemplateUsesUnsplitChoices,
    TooLongExtendedDescriptionInTemplates, TooLongShortDescriptionInTemplates,
    UnknownDebconfPriority, UnknownFieldInTemplates, UsingFirstPersonInTemplates,
    UsingImperativeFormInTemplates, UsingQuestionInExtendedDescriptionInTemplates,
};
pub(crate) use debian_readme::{
    ReadmeDebianContainsInvalidEmailAddress, ReadmeDebianMentionsUsrDoc,
};
pub(crate) use debian_rules::{
    BinaryArchRulesButPkgIsArchIndep, CustomCompressionInDebianRules,
    DebianRulesCallsDebhelperInOddOrder, DebianRulesCallsNproc, DebianRulesCallsPwd,
    DebianRulesIgnoresMakeCleanError, DebianRulesMissingRequiredTarget,
    DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
    OverrideDhFixpermsDoesNotCallDhFixperms,
};
pub(crate) use debian_shlibs::{
    DuplicateEntryInSymbolsControlFile, DuplicateInShlibs, InvalidTemplateIdInSymbolsFile,
    SymbolsFileContainsCurrentVersionWithDebianRevision, SymbolsFileContainsDebianRevision,
    SymbolsForUndeclaredSharedLibrary, SyntaxErrorInSymbolsFile,
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
pub(crate) use documentation_manual::{
    BadSoLinkWithinManualPage, BadWhatisEntry, BrokenLinkToUndocumented, BuildPathInManual,
    CountryInManual, EmptyManualPage, GroffMessage, ManualPageFromTemplate,
    ManualPageWithGenericName, NoEnglishManualPage, NoManualPage, PodConversionMessage,
    PoorCompressionInManualPage, UncompressedManualPage, UndocumentedManualPage,
    UselessWhatisEntry, WrongCompressionInManualPage,
};
pub(crate) use fields_package_relations::{
    BuildDependsOn1Revision, BuildDependsOnBuildEssential, BuildDependsOnMetapackage,
    BuildDependsOnObsoletePackage, DependsExclusivelyOnMakedev, DependsOnObsoletePackage,
    DependsOnOldEmacs, PackageRelationWithSelf, VirtualPackageDependsWithoutRealPackageDepends,
};
pub(crate) use fields_relations_more::{
    BadProvidedPackageName, BuildConflictsWithBuildDependency,
    BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
    BuildDependsOnEssentialPackageWithoutUsingVersion, BuildDependsOnObsoleteBootstrap,
    ConflictsWithDependency, DependsOnEssentialPackageWithoutUsingVersion, DependsOnMetapackage,
    DependsOnObsoleteBootstrap, DependsOnPackagingDev, DependsOnPythonMinimal,
    InvalidArchStringInSourceRelation, MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
    NeedlesslyDependsOnAwk, PackageDependsOnMultipleLibpngVersions,
    PackageDependsOnMultipleLibstdcVersions, PackageDependsOnMultipleTclVersions,
    PackageDependsOnMultipleTkVersions, PackageRelationWithPerlModules,
};
pub(crate) use fields_text::{
    BadHomepage, DescriptionContainsHomepage, DescriptionSynopsisIsDuplicated,
    EssentialNoNotNeeded, FieldTooLong, MissingSeparatorBetweenItems, NewEssentialPackage,
    RecommendedField, RedundantBugsField, RedundantOriginField, SuperfluousClutterInHomepage,
    UnknownSection,
};
pub(crate) use files_artifact::{
    DebianAddsArchControlDir, DebianAddsArchInventoryFile, DebianAddsBtsControlDir,
    DebianAddsBzrControlDir, DebianAddsCvsConflictCopy, DebianAddsCvsControlDir,
    DebianAddsEditorBackupFile, DebianAddsGitControlDir, DebianAddsHgControlDir,
    DebianAddsHgTagsFile, DebianAddsPatchFailureFile, DebianAddsQuiltControlDir,
    DebianAddsSvkCommitFile, DebianAddsSvnCommitFile, DebianAddsSvnConflictFile,
    DebianAddsSvnControlDir,
};
pub(crate) use files_foreign::{
    CompressedDocumentation, DebugSuffixNotDbg, FileDirectlyInUsrShareDoc,
    MacosDsStoreFileInPackage, MacosResourceForkFileInPackage, PackageContainsLindaOverride,
    PackageContainsReadmeForOtherPlatformOrDistro, PackageInstallsAppimage,
};
pub(crate) use files_hierarchy::{
    AliasedLocation, DirInUsrLocal, DirOrFileInEtcOpt, DirOrFileInHome, DirOrFileInMnt,
    DirOrFileInOpt, DirOrFileInRun, DirOrFileInSrv, DirOrFileInTmp, DirOrFileInVarLock,
    DirOrFileInVarRun, DirOrFileInVarWww, FileDirectlyInUsrShare, FileInUsrLocal, FsstndDirInUsr,
    FsstndDirInVar, NonMultiArchLibDir, NonStandardDirInUsr, NonStandardDirInVar, SubdirInUsrBin,
    UseOfCompatSymlink,
};
pub(crate) use files_includes::HeaderHasOverlyGenericName;
pub(crate) use files_installed::{
    FileInUsrLibSgml, PackageInstallsIntoEtcGconfSchemas, RunPartsCronFilenameContainsIllegalChars,
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
pub(crate) use games::{PackageSectionGamesButContainsNoGame, PackageSectionGamesButHasUsrBin};
pub(crate) use init_scripts::{
    InitDScriptDependsOnAllVirtualFacility, InitDScriptDoesNotImplementRequiredOption,
    InitDScriptHasBadStopRunlevel, InitDScriptHasConflictingStartStop,
    InitDScriptMissingLsbSection, InitDScriptPossibleMissingStop,
    InitDScriptShouldAlwaysStartService, InitDScriptStartsInStopRunlevel,
    InitDScriptStopsInSRunlevel, MissingSystemdServiceForInitDRcSScript,
    NonVirtualFacilityInInitdScript, SystemdServiceFileRefersToObsoleteTarget,
    SystemdServiceFileWrapsInitScript,
};
pub(crate) use languages_javascript::{PkgJsToolsTestIsEmpty, PkgJsToolsTestIsMissing};
pub(crate) use languages_perl::PackageInstallsNonbinaryPerlInUsrLibPerl5;
pub(crate) use languages_python::{
    AncientPythonVersionField, FileInUsrLibSitePython, MalformedPythonVersion,
    PackageInstallsDeprecatedPython2Path, PythonVersionCurrentIsDeprecated,
    SourcePackageEncodesPythonVersion, ThirdPartyPackageInPythonDir,
    UnknownFileInPythonModuleDirectory,
};
pub(crate) use mailcap::QuotedPlaceholderInMailcapEntry;
pub(crate) use md5sums::{
    FileMissingInMd5sums, MalformedMd5sumsControlFile, Md5sumMismatch, Md5sumsListsNonexistentFile,
};

pub(crate) use debian_patches::Format30ButDebianChangesPatch;
pub(crate) use debian_po_debconf::{
    StrayTranslatedDebconfTemplates, UntranslatableDebconfTemplates,
};
pub(crate) use debian_variables::IllegalVariable;
pub(crate) use fields_mail_address::{
    FaultyDebianQaGroupAddress, FaultyDebianQaGroupPhrase, MailAddressLoopsOrBounces,
};
pub(crate) use fields_version::{DebianRevisionIsZero, DebianRevisionNotWellFormed};
pub(crate) use ln0001_required_field::RequiredField;
pub(crate) use ln0002_syntax_error_in_debian_changelog::SyntaxErrorInDebianChangelog;
pub(crate) use ln0004_source_contains_prebuilt_windows_binary::SourceContainsPrebuiltWindowsBinary;
pub(crate) use ln0005_unstripped_binary_or_object::UnstrippedBinaryOrObject;
pub(crate) use ln0007_empty_binary_package::EmptyBinaryPackage;
pub(crate) use ln0008_superfluous_file_pattern::SuperfluousFilePattern;
pub(crate) use ln0011_file_in_unusual_dir::FileInUnusualDir;
pub(crate) use ln0012_no_changelog::NoChangelog;
pub(crate) use ln0013_debian_changelog_file_missing_or_wrong_name::DebianChangelogFileMissingOrWrongName;
pub(crate) use ln0014_maintainer_script_empty::MaintainerScriptEmpty;
pub(crate) use ln0015_source_contains_debian_substvars::SourceContainsDebianSubstvars;
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
pub(crate) use ln0183_package_contains_bts_control_dir::PackageContainsBtsControlDir;
pub(crate) use ln0184_uses_dpkg_database_directly::UsesDpkgDatabaseDirectly;
pub(crate) use ln0185_obsolete_url_in_packaging::ObsoleteUrlInPackaging;
pub(crate) use menu_format::{
    CommandInMenuFileAndDesktopFile, DesktopCommandNotInPackage, DesktopEntryContainsUnknownKey,
    DesktopEntryFileHasCrs, DesktopEntryInvalidCategory, DesktopEntryUsesReservedCategory,
    MenuCommandNotInPackage, MenuItemCreatesNewSection, MenuItemUsesAppsSection,
    MenuItemUsesWindowmanagersSection, NonWmInWindowmanagerMenuSection, PkgNotInPackageTest,
};
pub(crate) use menus::{
    DocBaseAbstractFieldSeparatorExtraWhitespace, DocBaseFileNoFormatSection,
    MenuMethodLacksInclude, PostinstDoesNotCallUpdatemenus, PostinstHasUselessCallToInstallDocs,
    PostrmDoesNotCallUpdatemenus, PrermHasUselessCallToInstallDocs,
};
pub(crate) use po_debconf::{InvalidPoFile, MisnamedPoFile, TranslatedDefaultField};
pub(crate) use rules_and_control::{
    BraceExpansionInDebhelperConfigFile, BuildPrerequisiteInInstallableSection,
    DebhelperToolsFromAutotoolsDevAreDeprecated, DebianControlHasDbgsymPackage,
    PackageUsesDhExecButLacksBuildDepends,
};
pub(crate) use scripts_calls::{
    MaintainerScriptCallsGconftool, MaintainerScriptCallsStartStopDaemon,
    MaintainerScriptNeedsDependsOnAdduser, MaintainerScriptNeedsDependsOnUpdateInetd,
};
pub(crate) use scripts_depends::{
    MaintainerScriptCallsInitScriptDirectly, MaintainerScriptNeedsDependsOnGconf2,
    MaintainerScriptNeedsDependsOnUcf, MaintainerScriptNeedsDependsOnXfontsUtils,
    MaintainerScriptNeedsDependsOnXmlCore, MaintainerScriptShouldNotParseEtcPasswdOrGroup,
};
pub(crate) use scripts_shell::{
    CommandWithPathInMaintainerScript, MaintainerScriptIgnoresErrors,
    MaintainerScriptRemovesDeviceFiles, ReadInMaintainerScript, RecursivePrivilegeChange,
};
pub(crate) use scripts_text::{
    ForbiddenConfigInterpreter, InstallInfoUsedInMaintainerScript,
    MaintainerScriptCallsChownImproperly, MaintainerScriptCallsInstallSgmlcatalog,
    MaintainerScriptChangesLdSoConf, MaintainerScriptChangesNetbase,
    MaintainerScriptHasInvalidUpdateInetdOptions, MaintainerScriptHasUnexpandedDebhelperToken,
    MaintainerScriptHidesInitFailure, MaintainerScriptModifiesInetdConf,
    MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
    MaintainerScriptShouldNotUsePiupartsVariable, MaintainerScriptUpdatesFontconfigCacheImproperly,
    MaintscriptCallsLdconfig, MissingCallToDpkgMaintscriptHelper, PackageUsesLocalDiversion,
    PossiblyInsecureHandlingOfTmpFilesInMaintainerScript, PostrmRemovesAlternative,
    ScriptUsesBinEnv, SystemdAlternatives, UdevadmCalledWithoutGuard,
};
pub(crate) use systemd::{
    InitDScriptDoesNotSourceInitFunctions, MissingSystemdServiceForInitDScript,
    SystemdServiceAliasWithoutExtension, SystemdServiceFileRefersToObsoleteBindto,
    SystemdServiceFileUsesNobodyOrNogroup,
};
pub(crate) use testsuite::{
    DebianTestsControlAutodep8IsObsolete, NoOpTestsuite,
    RuntimeTestFileUsesInstalledPythonVersions, UnnecessaryTestsuiteAutopkgtestField,
};
pub(crate) use triggers::UsesImplicitAwaitTrigger;
