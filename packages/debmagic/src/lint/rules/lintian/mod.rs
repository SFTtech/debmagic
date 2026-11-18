mod apache2;
mod archive_liberty;
mod autotools;
mod binaries_location;
mod binaries_perl;
mod binaries_rpath;
mod binaries_static;
mod changelog_doc;
mod conffiles;
mod debhelper_rules;
mod debian_changelog;
mod debian_copyright;
mod debian_debconf;
mod debian_line_separators;
mod debian_maintscript;
mod debian_patches;
mod debian_po_debconf;
mod debian_readme;
mod debian_rules;
mod debian_shlibs;
mod debian_source_dir;
mod debian_variables;
mod debian_version_substvars;
mod debian_watch;
mod description;
mod desktop;
mod desktop_gir;
mod documentation;
mod documentation_manual;
mod documentation_texinfo;
mod domain;
mod executable;
mod fields_mail_address;
mod fields_package_relations;
mod fields_relations_extra;
mod fields_relations_more;
mod fields_standards_version;
mod fields_text;
mod fields_version;
mod files_artifact;
mod files_build_path;
mod files_config_scripts;
mod files_debug;
mod files_foreign;
mod files_hard_links;
mod files_hierarchy;
mod files_ieee;
mod files_includes;
mod files_installed;
mod files_multi_arch;
mod files_names;
mod files_non_free;
mod files_permissions;
mod files_pkgconfig;
mod files_privacy_breach;
mod files_symbolic_links;
mod files_vcs;
mod fonts_opentype;
mod fonts_truetype;
mod games;
mod init_scripts;
mod known_fields;
mod languages_javascript;
mod languages_perl;
mod languages_php;
mod languages_python;
mod languages_r;
mod libraries_shared_soname;
mod lintian_overrides;
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
mod maintainer_scripts_diversion;
mod md5sums;
mod menu_format;
mod menus;
mod modprobe;
mod nodejs;
mod po_debconf;
mod rules_and_control;
mod scripts_calls;
mod scripts_depends;
mod scripts_interpreters;
mod scripts_shell;
mod scripts_text;
mod systemd;
mod testsuite;
mod tlds;
mod triggers;
mod vcs;

pub(crate) use apache2::{
    Apache2ConfigurationFilesNeedConfSuffix, Apache2ReverseDependencyCallsWrapperScript,
    Apache2UnparsableDependency, NonStandardApache2ConfigurationName,
};
pub(crate) use archive_liberty::ArchiveLibertyMismatch;
pub(crate) use autotools::{
    AncientLibtool, AutotoolsPkgConfigMacroNotCrossCompilationSafe, ConfigureGeneratedFileInSource,
};
pub(crate) use binaries_location::{
    ArchDependentFileInUsrShare, ArchDependentFileNotInArchSpecificDirectory,
};
pub(crate) use binaries_perl::MissingDependencyOnPerlapi;
pub(crate) use binaries_rpath::RelativeLibrarySearchPath;
pub(crate) use binaries_static::StaticallyLinkedBinary;
pub(crate) use conffiles::{
    FileInUsrMarkedAsConffile, MissingConffile, NonEtcFileMarkedAsConffile,
};
pub(crate) use debhelper_rules::{
    DebhelperBaseCompatLevelIsMissing, DebhelperButNoMiscDepends,
    MaintainerScriptLacksDebhelperToken, MissingBuildDependencyForDhAddon,
    MissingBuildDependencyForDhCommand, NoVersionedDebhelperPrerequisite,
    TypoInDebhelperOverrideTarget,
};
pub(crate) use desktop::{
    DesktopFileInWrongDir, FileInDiscouragedX11FontDirectory, FileInUnknownX11FontDirectory,
    IconSizeAndDirectoryNameMismatch, PackageContainsCompiledFontFile,
    PackageInstallsFileToUsrX11r6,
};
pub(crate) use desktop_gir::GirSectionNotLibdevel;
pub(crate) use documentation::ZeroByteFileInDocDirectory;

pub(crate) use debian_changelog::{
    BogusMailHostInDebianChangelog, ChangelogEmptyEntry, ChangelogNewsDebianMismatch,
    ChangelogReferencesTempSecurityIdentifier,
    DebianChangelogFileContainsObsoleteUserEmacsSettings, DebianChangelogHasWrongDayOfWeek,
    DebianChangelogLineTooLong, DebianNewsEntryHasStrangeDistribution,
    DebianNewsEntryHasUnknownVersion, DebianNewsFileNotCompressed, EpochChangeWithoutComment,
    EpochChangedButUpstreamVersionDidNotGoBackwards, HtmlChangelogWithoutTextVersion,
    ImprobableBugNumberInCloses, InitialUploadClosesNoBugs, InvalidDateInDebianChangelog,
    LatestChangelogEntryWithoutNewDate, LatestDebianChangelogEntryReusesExistingVersion,
    NoNmuInChangelog, NoQaInChangelog, OddHistoricalDebianChangelogVersion,
    QaUploadHasIncorrectVersionNumber, SourceNmuHasIncorrectVersionNumber,
    SyntaxErrorInDebianNewsFile, UploadersInOrphan, WrongNameForDebianNewsFile,
};
pub(crate) use debian_copyright::{
    AmbiguousParagraphInDep5Copyright, BadExceptionFormatInDep5Copyright,
    CopyrightContainsAutomaticallyExtractedBoilerplate, CopyrightContainsDhMakeTodoBoilerplate,
    CopyrightDoesNotReferToCommonLicenseFile, CopyrightFileCompressed,
    CopyrightFileContainsFullApache2License, CopyrightFileContainsFullGfdlLicense,
    CopyrightFileContainsFullGplLicense, CopyrightFileIsSymlink,
    CopyrightFileLacksPointerToPerlLicense, CopyrightHasUrlFromDhMakeBoilerplate,
    CopyrightNotUsingCommonLicenseForApache2, CopyrightNotUsingCommonLicenseForGfdl,
    CopyrightNotUsingCommonLicenseForGpl, CopyrightNotUsingCommonLicenseForLgpl,
    CopyrightRefersToCompressedLicense, CopyrightRefersToDeprecatedBsdLicenseFile,
    CopyrightRefersToIncorrectDirectory, CopyrightRefersToNonexistentLicenseFile,
    CopyrightRefersToOldDirectory, CopyrightRefersToVersionlessLicenseFile,
    CopyrightWithoutCopyrightNotice, DebianCopyrightIsSymlink, Dep5CopyrightLicenseNameNotUnique,
    Dep5FileParagraphReferencesHeaderParagraph, EmptyShortLicenseInDep5Copyright,
    GlobalFilesWildcardNotFirstParagraphInDep5Copyright, HelperTemplatesInCopyright,
    IncompleteCreativeCommonsLicense, InvalidShortNameInDep5Copyright,
    LicenseProblemUndefinedLicense, MissingFieldInDep5Copyright,
    MissingLicenseParagraphInDep5Copyright, MissingLicenseTextInDep5Copyright,
    NamedCopyrightForSingleInstallable, NoCopyrightFile, NoDebianCopyrightInSource,
    NoDep5Copyright, OldFsfAddressInCopyrightFile, PipeSymbolUsedAsLicenseDisjunction,
    SourceShipsExcludedFile, SpaceInStdShortnameInDep5Copyright,
    UnusedLicenseParagraphInDep5Copyright,
};

pub(crate) use debian_debconf::{
    BooleanTemplateHasBogusDefault, DuplicateLongDescriptionInTemplate, EmptyTranslatedChoices,
    LoadsObsoleteConfmodule, MakingAssumptionsAboutInterfacesInTemplates,
    MalformedPromptInTemplates, MalformedQuestionInTemplates, MalformedTitleInTemplates,
    MismatchTranslatedChoices, MissingDebconfDependencyForPreinst, NoDebconfTemplates,
    NoTemplateDescription, NoTemplateType, PossibleDebconfNoteAbuse, PostinstDoesNotLoadConfmodule,
    PostinstUsesDbInput, SelectWithBooleanChoices, TemplateUsesUnsplitChoices,
    TooLongExtendedDescriptionInTemplates, TooLongShortDescriptionInTemplates,
    UnknownDebconfPriority, UnknownFieldInTemplates, UsingFirstPersonInTemplates,
    UsingImperativeFormInTemplates, UsingQuestionInExtendedDescriptionInTemplates,
};
pub(crate) use debian_line_separators::CarriageReturnLineFeed;
pub(crate) use debian_readme::{
    ReadmeDebianContainsInvalidEmailAddress, ReadmeDebianMentionsUsrDoc,
};
pub(crate) use debian_rules::{
    BinaryArchRulesButPkgIsArchIndep, CustomCompressionInDebianRules,
    DebianRulesCallsDebhelperInOddOrder, DebianRulesCallsNproc, DebianRulesCallsPwd,
    DebianRulesIgnoresMakeCleanError, DebianRulesIsDhMakeTemplate,
    DebianRulesMissingRequiredTarget, DebianRulesPassesVersionInfoToDhShlibdeps,
    DebianRulesSetsDebBuildOptions, DebianRulesSetsDhCompat,
    DebianRulesSetsDpkgArchitectureVariable, DebianRulesShouldNotUseDhExtraAddons,
    DebianRulesShouldNotUseSanitizeAllBuildflag, DebianRulesUpdatesControlAutomatically,
    DebianRulesUsesDebBuildOpts, DebianRulesUsesInstalledPythonVersions,
    DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
    OverrideDhFixpermsDoesNotCallDhFixperms, RulesRequireBuildPrerequisite,
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
    DescriptionContainsDhMakePerlTemplate, DescriptionContainsInvalidControlStatement,
    DescriptionContainsTabs, DescriptionIsDebmakeTemplate, DescriptionIsPkgName,
    DescriptionStartsWithLeadingSpaces, DescriptionStartsWithPackageName,
    DescriptionSynopsisIsEmpty, DescriptionSynopsisStartsWithArticle, DescriptionTooShort,
    ExtendedDescriptionContainsEmptyParagraph, ExtendedDescriptionIsEmpty,
    ExtendedDescriptionLineTooLong, SynopsisTooLong,
};
pub(crate) use documentation_manual::{
    BadSoLinkWithinManualPage, BadWhatisEntry, BrokenLinkToUndocumented, BuildPathInManual,
    CountryInManual, EmptyManualPage, GroffMessage, ManualPageFromTemplate,
    ManualPageWithGenericName, NoEnglishManualPage, NoManualPage, PodConversionMessage,
    PoorCompressionInManualPage, UncompressedManualPage, UndocumentedManualPage,
    UselessWhatisEntry, WrongCompressionInManualPage,
};
pub(crate) use documentation_texinfo::{
    InfoDocumentMissingDirEntry, InfoDocumentMissingDirSection,
};
pub(crate) use executable::ExecutableNotElfOrScript;
pub(crate) use fields_package_relations::{
    BuildDependsOn1Revision, BuildDependsOnBuildEssential, BuildDependsOnMetapackage,
    BuildDependsOnObsoletePackage, DependsExclusivelyOnMakedev, DependsOnObsoletePackage,
    DependsOnOldEmacs, NeedlessSuggestRecommendLibservletJava, PackageRelationWithSelf,
    VirtualPackageDependsWithoutRealPackageDepends,
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
    EssentialInSourcePackage, EssentialNoNotNeeded, ExcessivePriorityForLibraryPackage,
    ExplicitDefaultInPackageType, FieldTooLong, HomepageForCpanPackageContainsVersion,
    HomepageRefersToObsoleteDebianInfrastructure, MissingSeparatorBetweenItems,
    NewEssentialPackage, RecommendedField, RedundantBugsField, RedundantOriginField,
    SuperfluousClutterInHomepage, UnknownSection,
};
pub(crate) use files_artifact::{
    DebianAddsArchControlDir, DebianAddsArchInventoryFile, DebianAddsBtsControlDir,
    DebianAddsBzrControlDir, DebianAddsCvsConflictCopy, DebianAddsCvsControlDir,
    DebianAddsEditorBackupFile, DebianAddsGitControlDir, DebianAddsHgControlDir,
    DebianAddsHgTagsFile, DebianAddsPatchFailureFile, DebianAddsQuiltControlDir,
    DebianAddsSvkCommitFile, DebianAddsSvnCommitFile, DebianAddsSvnConflictFile,
    DebianAddsSvnControlDir,
};
pub(crate) use files_build_path::DirOrFileInBuildTree;
pub(crate) use files_config_scripts::{
    OldStyleConfigScriptMultiarchPath, OldStyleConfigScriptMultiarchPathArchAll,
};
pub(crate) use files_debug::NonDebugFileInDebugPackage;
pub(crate) use files_foreign::{
    CompressedDocumentation, DebugSuffixNotDbg, FileDirectlyInUsrShareDoc,
    MacosDsStoreFileInPackage, MacosResourceForkFileInPackage, PackageContainsLindaOverride,
    PackageContainsReadmeForOtherPlatformOrDistro, PackageInstallsAppimage,
    WindowsThumbnailDatabaseInPackage,
};
pub(crate) use files_hard_links::PackageContainsHardlink;
pub(crate) use files_hierarchy::{
    AliasedLocation, DirInUsrLocal, DirOrFileInEtcOpt, DirOrFileInHome, DirOrFileInMnt,
    DirOrFileInOpt, DirOrFileInRun, DirOrFileInSrv, DirOrFileInTmp, DirOrFileInVarLock,
    DirOrFileInVarRun, DirOrFileInVarWww, FileDirectlyInUsrShare, FileInUsrLocal, FsstndDirInUsr,
    FsstndDirInVar, NonMultiArchLibDir, NonStandardDirInUsr, NonStandardDirInVar, SubdirInUsrBin,
    UseOfCompatSymlink,
};
pub(crate) use files_ieee::PackageInstallsIeeeData;
pub(crate) use files_includes::HeaderHasOverlyGenericName;
pub(crate) use files_installed::{
    FileInUsrLibSgml, PackageInstallsIntoEtcGconfSchemas, RunPartsCronFilenameContainsIllegalChars,
};
pub(crate) use files_multi_arch::MultiarchForeignPkgconfig;
pub(crate) use files_names::{
    FileNameContainsWildcardCharacter, FileNameEndsInWhitespace, FileNameInPathIsNotAscii,
    HyphenFile, PackageContainsCompiledGlibSchema, PackageContainsEslintConfigFile,
    PackageContainsFileInEtcSkel, PackageContainsFileInUsrShareHal,
    PackageContainsIconCacheInGenericDir, PackageContainsNpmIgnoreFile,
    PackageContainsPythonCoverageFile, PackageContainsPythonDoctreeFile,
    PackageContainsPythonDotDirectory, PackageContainsPythonHeaderInIncorrectDirectory,
    PackageContainsPythonHypothesisExample, PackageContainsPythonTestsInGlobalNamespace,
    PackageContainsSassCacheDirectory, ShippedFileWithoutUtf8Name, StarFile,
    ZeroByteExecutableInPath,
};
pub(crate) use files_permissions::NonStandardExecutablePerm;
pub(crate) use files_pkgconfig::{
    PkgConfigBadDirective, PkgConfigMultiArchWrongDir, PkgConfigUnavailableForCrossCompilation,
};
pub(crate) use files_symbolic_links::{
    AbsoluteSymbolicLinkTargetInSource, AbsoluteSymlinkInTopLevelFolder,
    CompressedSymlinkWithWrongExt, LengthySymlink, PackageContainsUnsafeSymlink, RelativeSymlink,
    SymlinkContainsSpuriousSegments, SymlinkEndsWithSlash, SymlinkHasDoubleSlash,
    SymlinkHasTooManyUpSegments, SymlinkIsSelfRecursive,
};
pub(crate) use files_vcs::{
    PackageContainsVcsControlDir, PackageContainsVcsControlFile, SvkCommitFileInPackage,
    SvnCommitFileInPackage,
};
pub(crate) use fonts_opentype::{
    OpentypeFontProhibitsInstallableEmbedding, OpentypeFontWrongFilename,
};
pub(crate) use fonts_truetype::TruetypeFontProhibitsInstallableEmbedding;
pub(crate) use games::{
    GameOutsideSection, GlobalDataInGamesDirectory, PackageSectionGamesButContainsNoGame,
    PackageSectionGamesButHasUsrBin,
};
pub(crate) use init_scripts::{
    InitDScriptDependsOnAllVirtualFacility, InitDScriptDoesNotImplementRequiredOption,
    InitDScriptHasBadLsbLine, InitDScriptHasBadStopRunlevel, InitDScriptHasConflictingStartStop,
    InitDScriptMissingLsbSection, InitDScriptPossibleMissingStop,
    InitDScriptShouldAlwaysStartService, InitDScriptStartsInStopRunlevel,
    InitDScriptStopsInSRunlevel, MissingSystemdServiceForInitDRcSScript,
    NonVirtualFacilityInInitdScript, SystemdServiceFileRefersToObsoleteTarget,
    SystemdServiceFileWrapsInitScript,
};
pub(crate) use languages_javascript::{
    EmbeddedJavascriptLibrary, NodePackageInstallInNodejsRootdir, NodejsLockFile,
    NodejsModuleInstalledInUsrLib, PkgJsToolsTestIsEmpty, PkgJsToolsTestIsMissing,
};
pub(crate) use languages_perl::PackageInstallsNonbinaryPerlInUsrLibPerl5;
pub(crate) use languages_python::{
    AncientPythonVersionField, BogusPythonPrerequisite, EmbeddedFeedparserLibrary,
    FileInUsrLibSitePython, MalformedPythonVersion, PackageInstallsDeprecatedPython2Path,
    PythonModuleHasOverlyGenericName, PythonPackageMissingDependsOnPython,
    PythonVersionCurrentIsDeprecated, SourcePackageEncodesPythonVersion,
    ThirdPartyPackageInPythonDir, UnknownFileInPythonModuleDirectory,
};
pub(crate) use libraries_shared_soname::SharedobjectInLibraryDirectoryMissingSoname;
pub(crate) use mailcap::QuotedPlaceholderInMailcapEntry;
pub(crate) use md5sums::{
    FileMissingInMd5sums, MalformedMd5sumsControlFile, Md5sumMismatch, Md5sumsListsNonexistentFile,
};

pub(crate) use debian_maintscript::MaintscriptIncludesMaintScriptParameters;
pub(crate) use debian_patches::{
    Format30ButDebianChangesPatch, PatchFilePresentButNotMentionedInSeries,
    PatchSystemButNoSourceReadme,
};
pub(crate) use debian_po_debconf::{
    StrayTranslatedDebconfTemplates, UntranslatableDebconfTemplates,
};
pub(crate) use debian_source_dir::CustomCompressionInDebianSourceOptions;
pub(crate) use debian_variables::IllegalVariable;
pub(crate) use fields_mail_address::{
    FaultyDebianQaGroupAddress, FaultyDebianQaGroupPhrase, MailAddressLoopsOrBounces, RootInContact,
};
pub(crate) use fields_relations_extra::{
    BadVersionInRelation, BinaryPackageDependsOnToolchainPackage, BreaksWithoutVersion,
    BuildDependsArchWithoutArchDependentBinary, BuildDependsIndepWithoutArchIndep,
    BuildDependsOnAnObsoleteJavaPackage, BuildDependsOnDhSysuser, BuildDependsOnNonBuildPackage,
    BuildDependsOnSpecificJavaDocPackage, BuildDependsOnVersionedBerkeleyDb,
    ConflictingNegationInSourceRelation, DbgPackageMissingDepends,
    DefaultMtaDependencyDoesNotSpecifyMailTransportAgent, DefaultMtaDependencyNotListedFirst,
    DependsOnLibdb1Compat, DependsOnSpecificJavaDocPackage, DocPackageDependsOnMainPackage,
    ObsoleteRelationFormInSource, RedundantInstallationPrerequisite, WeakLibraryDevDependency,
};
pub(crate) use fields_standards_version::TimewarpStandardsVersion;
pub(crate) use fields_version::{DebianRevisionIsZero, DebianRevisionNotWellFormed};
pub(crate) use files_non_free::NonFreeFlash;
pub(crate) use files_privacy_breach::{
    PrivacyBreachDonation, PrivacyBreachFacebook, PrivacyBreachGeneric, PrivacyBreachGoogleAdsense,
    PrivacyBreachGoogleCse, PrivacyBreachGooglePlus, PrivacyBreachLogo, PrivacyBreachPiwik,
    PrivacyBreachStatisticsWebsite, PrivacyBreachTwitter, PrivacyBreachUsesEmbeddedFile,
};
pub(crate) use languages_php::EmbeddedPhpLibrary;
pub(crate) use languages_r::RDataWithoutReadmeSource;
pub(crate) use lintian_overrides::{AlienTag, OverrideFileInWrongLocation};
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
pub(crate) use maintainer_scripts_diversion::{
    DiversionForUnknownFile, OrphanedDiversion, PackageUsesLocalDiversion, RemoveOfUnknownDiversion,
};
pub(crate) use menu_format::{
    BadTestInMenuItem, CommandInMenuFileAndDesktopFile, DesktopCommandNotInPackage,
    DesktopContainsDeprecatedKey, DesktopEntryContainsDeprecatedKey,
    DesktopEntryContainsUnknownKey, DesktopEntryFileHasCrs, DesktopEntryInvalidCategory,
    DesktopEntryLacksMainCategory, DesktopEntryMissingRequiredKey, DesktopEntryUnknownType,
    DesktopEntryUsesReservedCategory, DesktopMimeButNoExecCode, DuplicateKeyInDesktop,
    DuplicateTagInMenu, MenuCommandNotInPackage, MenuIconMissing, MenuIconUsesRelativePath,
    MenuItemCreatesNewSection, MenuItemMissingRequiredTag, MenuItemNeedsTagHasUnknownValue,
    MenuItemUsesAppsSection, MenuItemUsesWindowmanagersSection, NonWmInWindowmanagerMenuSection,
    NonWmModuleInWmModulesMenuSection, PkgNotInPackageTest, SuToRootWithUsrSbin,
    SuWrapperNotSuToRoot, SuWrapperWithoutC, UnquotedStringInMenuItem,
};
pub(crate) use menus::{
    DocBaseAbstractFieldIsTemplate, DocBaseAbstractFieldSeparatorExtraWhitespace,
    DocBaseAbstractMightContainExtraLeadingWhitespace, DocBaseDocumentFieldEndsInWhitespace,
    DocBaseDocumentFieldNotInFirstLine, DocBaseFileLacksRequiredField, DocBaseFileNoFormat,
    DocBaseFileNoFormatSection, DocBaseFileNoIndex, DocBaseFileReferencesWrongPath,
    DocBaseFileSeparatorExtraWhitespace, DocBaseFileSyntaxError, DocBaseFileUnknownField,
    DocBaseFileUnknownFormat, DocBaseIndexReferencesMultipleFiles, DocBaseInvalidDocumentField,
    DocBaseUnknownSection, DocBaseUsesApplicationsSection, DuplicateFieldInDocBase,
    DuplicateFormatInDocBase, MenuMethodLacksInclude, PostinstDoesNotCallUpdatemenus,
    PostinstHasUselessCallToInstallDocs, PostrmDoesNotCallUpdatemenus,
    PrermHasUselessCallToInstallDocs,
};
pub(crate) use modprobe::ObsoleteCommandInModprobeDFile;
pub(crate) use nodejs::{NodejsMissingVersionOverride, NodejsModuleNotDeclared};
pub(crate) use po_debconf::{InvalidPoFile, MisnamedPoFile, TranslatedDefaultField};
pub(crate) use rules_and_control::{
    BraceExpansionInDebhelperConfigFile, BuildPrerequisiteInInstallableSection,
    CircularInstallationPrerequisite, DebhelperCompatFileContainsMultipleLevels,
    DebhelperCompatFileIsEmpty, DebhelperToolsFromAutotoolsDevAreDeprecated,
    DebianControlHasDbgsymPackage, DebianControlRepeatsFieldNameInValue,
    DebianRulesUsesDeprecatedSystemdOverride, DebianRulesUsesUnnecessaryDhArgument,
    DeclaresPossiblyConflictingDebhelperCompatVersions, DhCleanKIsDeprecated,
    DhExecInstallNotAllowedHere, DhExecScriptWithoutDhExecFeatures, DhInstallmanpagesIsObsolete,
    DhQuiltAddonButQuiltSourceFormat, ExcessiveDebhelperOverrides,
    PackageUsesDebhelperButLacksBuildDepends, PackageUsesDhExecButLacksBuildDepends,
    TemporaryDebhelperFile, UselessAutoreconfBuildDepends,
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
pub(crate) use scripts_interpreters::{
    MissingDepForInterpreter, PhpScriptButNoPhpCliDep, ScriptWithoutInterpreter,
    UnusualInterpreter, WrongPathForInterpreter,
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
    MaintscriptCallsLdconfig, MissingCallToDpkgMaintscriptHelper,
    PossiblyInsecureHandlingOfTmpFilesInMaintainerScript, PostrmRemovesAlternative,
    ScriptUsesBinEnv, SystemdAlternatives, UdevadmCalledWithoutGuard,
};
pub(crate) use systemd::{
    InitDScriptDoesNotSourceInitFunctions, MissingSystemdServiceForInitDScript,
    MissingSystemdTimerForCronScript, SystemdServiceAliasWithoutExtension,
    SystemdServiceFileRefersToObsoleteBindto, SystemdServiceFileUsesNobodyOrNogroup,
};
pub(crate) use testsuite::{
    BackgroundedTestCommand, ConflictingTestFields, DebianTestsControlAutodep8IsObsolete,
    DeclarePythonVersionsForTest, DropPythonVersionDeclaration, EmptyDebianTestsControl,
    IllegalRuntimeTestName, MissingRuntimeTestFile, NoOpTestsuite,
    RuntimeTestFileUsesInstalledPythonVersions,
    RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends,
    TestsuiteDependsOnObsoletePackage, UnnecessaryTestsuiteAutopkgtestField,
};
pub(crate) use triggers::UsesImplicitAwaitTrigger;
