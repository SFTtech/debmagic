mod apache2;
mod application_not_library;
mod apt;
mod archive_liberty;
mod autotools;
mod binaries_architecture;
mod binaries_corrupted;
mod binaries_debug_symbols_detached;
mod binaries_hardening;
mod binaries_location;
mod binaries_obsolete_crypt;
mod binaries_perl;
mod binaries_prerequisites_php;
mod binaries_profiling;
mod binaries_rpath;
mod binaries_static;
mod build_systems_waf;
mod changelog_doc;
mod conffiles;
mod control_files;
mod deb_format;
mod debhelper_rules;
mod debian_changelog;
mod debian_control_field_architecture_multiline;
mod debian_control_field_build_profiles;
mod debian_control_link;
mod debian_copyright;
mod debian_debconf;
mod debian_dpkg_build_api;
mod debian_filenames;
mod debian_line_separators;
mod debian_lintian_overrides_restricted;
mod debian_maintscript;
mod debian_not_installed;
mod debian_patches;
mod debian_po_debconf;
mod debian_readme;
mod debian_rules;
mod debian_shlibs;
mod debian_source_dir;
mod debian_symbols;
mod debian_trailing_whitespace;
mod debian_upstream;
mod debian_variables;
mod debian_version_substvars;
mod debian_watch;
mod description;
mod desktop;
mod desktop_gir;
mod dh_make;
mod documentation;
mod documentation_devhelp_standard;
mod documentation_examples;
mod documentation_manual;
mod documentation_texinfo;
mod domain;
mod emacs;
mod emacs_elpa;
mod executable;
mod fields_architecture;
mod fields_bugs;
mod fields_built_using;
mod fields_empty;
mod fields_format;
mod fields_installer_menu_item;
mod fields_mail_address;
mod fields_maintainer_team;
mod fields_multi_arch;
mod fields_package;
mod fields_package_relations;
mod fields_relations_extra;
mod fields_relations_more;
mod fields_source;
mod fields_standards_version;
mod fields_static_built_using;
mod fields_text;
mod fields_uploaders;
mod fields_vcs;
mod fields_version;
mod files_architecture;
mod files_artifact;
mod files_banned_compiled_help;
mod files_build_path;
mod files_compressed_gz;
mod files_compressed_lzma;
mod files_compressed_xz;
mod files_config_scripts;
mod files_debug;
mod files_duplicates;
mod files_foreign;
mod files_hard_links;
mod files_hierarchy;
mod files_ieee;
mod files_includes;
mod files_init;
mod files_installed;
mod files_ld_so;
mod files_missing;
mod files_multi_arch;
mod files_names;
mod files_non_free;
mod files_obsolete_paths;
mod files_p11_kit;
mod files_pam;
mod files_permissions;
mod files_pkgconfig;
mod files_privacy_breach;
mod files_scripts;
mod files_special;
mod files_symbolic_links;
mod files_unwanted;
mod files_vcs;
mod fonts;
mod fonts_opentype;
mod fonts_truetype;
mod games;
mod images_thumbnails;
mod includes_config_h;
mod init_scripts;
mod known_fields;
mod languages_fortran_gfortran;
mod languages_golang_module_path;
mod languages_java;
mod languages_java_bytecode;
mod languages_javascript;
mod languages_ocaml;
mod languages_perl;
mod languages_php;
mod languages_puppet;
mod languages_python;
mod languages_r;
mod languages_ruby;
mod languages_rust;
mod libraries_debug_symbols;
mod libraries_shared_file_permissions;
mod libraries_shared_multi_arch;
mod libraries_shared_relocation;
mod libraries_shared_soname;
mod libraries_shared_stack;
mod libraries_static;
mod libtool;
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
mod ln0858_package_contains_cmake_private_file;
mod ln0927_missing_depends_on_sensible_utils;
mod ln1086_source_contains_prebuilt_silverlight_object;
mod mailbox;
mod mailcap;
mod maintainer_scripts_adduser;
mod maintainer_scripts_diversion;
mod maintainer_scripts_dpkg_statoverride;
mod maintainer_scripts_killall;
mod maintainer_scripts_mknod;
mod maintainer_scripts_systemctl;
mod md5sums;
mod menu_format;
mod menus;
mod mimeinfo;
mod modprobe;
mod nodejs;
mod origtar;
mod po_debconf;
mod rules_and_control;
mod scripts_calls;
mod scripts_depends;
mod scripts_interpreters;
mod scripts_shell;
mod scripts_text;
mod shell_csh;
mod substvars_libc;
mod substvars_misc_pre_depends;
mod systemd;
mod template_dh_make_control;
mod testsuite;
mod tlds;
mod triggers;
mod usrmerge;
mod vcs;
mod vim;

pub(crate) use apache2::{
    Apache2ConfigurationFilesNeedConfSuffix, Apache2DeprecatedAuthConfig,
    Apache2ModuleDoesNotDependOnApache2Api, Apache2ModuleDoesNotShipLoadFile,
    Apache2ReverseDependencyCallsInvokeRcD, Apache2ReverseDependencyCallsWrapperScript,
    Apache2ReverseDependencyShipsFileInNotAllowedDirectory,
    Apache2ReverseDependencyUsesObsoleteDirectory, Apache2UnparsableDependency,
    Apache2UnsupportedDependency, NonStandardApache2ConfigurationName,
    NonStandardApache2ModulePackageName, WebApplicationDependsOnApache2DataPackage,
    WebApplicationWorksOnlyWithApache,
};
pub(crate) use application_not_library::LibappPerlPackageName;
pub(crate) use apt::PackageInstallsAptKeyring;
pub(crate) use apt::PackageInstallsAptPreferences;
pub(crate) use apt::PackageInstallsAptSources;
pub(crate) use archive_liberty::ArchiveLibertyMismatch;
pub(crate) use autotools::{
    AncientLibtool, AutotoolsPkgConfigMacroNotCrossCompilationSafe, ConfigureGeneratedFileInSource,
};
pub(crate) use binaries_architecture::ArchIndependentPackageContainsBinaryOrObject;
pub(crate) use binaries_corrupted::BinaryWithBadDynamicTable;
pub(crate) use binaries_debug_symbols_detached::DebugSymbolsDirectlyInUsrLibDebug;
pub(crate) use binaries_debug_symbols_detached::DebugSymbolsNotDetached;
pub(crate) use binaries_hardening::HardeningNoBindnow;
pub(crate) use binaries_hardening::HardeningNoFortifyFunctions;
pub(crate) use binaries_hardening::HardeningNoPie;
pub(crate) use binaries_hardening::HardeningNoRelro;
pub(crate) use binaries_location::{
    ArchDependentFileInUsrShare, ArchDependentFileNotInArchSpecificDirectory, BinaryInEtc,
};
pub(crate) use binaries_obsolete_crypt::ObsoleteCryptAlias;
pub(crate) use binaries_obsolete_crypt::ObsoleteDesEncryption;
pub(crate) use binaries_perl::MissingDependencyOnPerlapi;
pub(crate) use binaries_prerequisites_php::MissingDependencyOnPhpapi;
pub(crate) use binaries_profiling::BinaryCompiledWithProfilingEnabled;
pub(crate) use binaries_rpath::CustomLibrarySearchPath;
pub(crate) use binaries_rpath::RelativeLibrarySearchPath;
pub(crate) use binaries_static::StaticallyLinkedBinary;
pub(crate) use build_systems_waf::SourceContainsWafBinary;
pub(crate) use conffiles::{
    ConffileHasBadFileType, DuplicateConffile, FileInEtcRcDMarkedAsConffile,
    FileInUsrMarkedAsConffile, MissingConffile, NonEtcFileMarkedAsConffile, RelativeConffile,
    UnexpectedConffile, UnknownConffileInstruction,
};
pub(crate) use control_files::ControlFileHasBadOwner;
pub(crate) use control_files::ControlFileHasBadPermissions;
pub(crate) use control_files::ControlFileIsEmpty;
pub(crate) use control_files::ControlFileIsNotAFile;
pub(crate) use control_files::NotAllowedControlFile;
pub(crate) use deb_format::MisplacedExtraMemberInDeb;
pub(crate) use deb_format::UdebUsesUnsupportedCompressionForDataTarball;
pub(crate) use deb_format::UsesNoCompressionForControlTarball;
pub(crate) use deb_format::UsesNoCompressionForDataTarball;
pub(crate) use debhelper_rules::{
    DebhelperBaseCompatLevelIsMissing, DebhelperButNoMiscDepends,
    MaintainerScriptLacksDebhelperToken, MissingBuildDependencyForDhAddon,
    MissingBuildDependencyForDhCommand, NoVersionedDebhelperPrerequisite,
    TypoInDebhelperOverrideTarget,
};
pub(crate) use debian_control_field_architecture_multiline::MultilineArchitectureField;
pub(crate) use debian_control_field_build_profiles::InvalidProfileNameInBuildProfilesField;
pub(crate) use debian_control_field_build_profiles::VariesContentWithNocheck;
pub(crate) use debian_control_link::DebianControlFileIsASymlink;
pub(crate) use debian_dpkg_build_api::DebianRulesDefinesDpkgBuildApi;
pub(crate) use debian_lintian_overrides_restricted::InvalidOverrideRestriction;
pub(crate) use debian_not_installed::UnwantedPathTooSpecific;
pub(crate) use debian_symbols::PackagePlaceholderInSymbolsFile;
pub(crate) use debian_trailing_whitespace::NoNewlineAtEnd;
pub(crate) use debian_upstream::DebianUpstreamObsoletePath;
pub(crate) use debian_upstream::UpstreamMetadataInNativeSource;
pub(crate) use debian_upstream::UpstreamMetadataIsNotAFile;
pub(crate) use desktop::{
    DesktopFileInWrongDir, FileInDiscouragedX11FontDirectory, FileInUnknownX11FontDirectory,
    IconSizeAndDirectoryNameMismatch, MissingCallToUpdateFonts, PackageContainsCompiledFontFile,
    PackageContainsMultipleDpiFonts, PackageInstallsFileToUsrX11r6, PackageMixesMiscAndDpiFonts,
    RasterImageInScalableDirectory,
};
pub(crate) use desktop_gir::GirInArchAllPackage;
pub(crate) use desktop_gir::GirPackageNameDoesNotMatch;
pub(crate) use desktop_gir::GirSectionNotLibdevel;
pub(crate) use desktop_gir::TypelibMissingGirDepends;
pub(crate) use desktop_gir::TypelibNotInMultiarchDirectory;
pub(crate) use desktop_gir::TypelibPackageNameDoesNotMatch;
pub(crate) use desktop_gir::TypelibSectionNotIntrospection;
pub(crate) use dh_make::ReadmeSourceIsDhMakeTemplate;
pub(crate) use documentation::{ExecutableInUsrShareDoc, ZeroByteFileInDocDirectory};
pub(crate) use documentation_devhelp_standard::OldDevhelpStandard;
pub(crate) use fields_empty::EmptyField;
pub(crate) use fields_format::UnsupportedSourceFormat;
pub(crate) use fields_maintainer_team::WrongTeam;
pub(crate) use fields_multi_arch::FontPackageNotMultiArchForeign;
pub(crate) use fields_multi_arch::IllegalMultiArchValue;
pub(crate) use fields_multi_arch::MultiArchSamePackageHasArchSpecificOverrides;
pub(crate) use fields_multi_arch::UnknownMultiArchValue;
pub(crate) use fields_package::BadPackageName;
pub(crate) use fields_package::PackageNotLowercase;
pub(crate) use fields_source::SourceFieldMalformed;
pub(crate) use files_architecture::TripletDirAndArchitectureMismatch;
pub(crate) use files_banned_compiled_help::SourceContainsPrebuiltMsHelpFile;
pub(crate) use files_compressed_gz::BrokenGz;
pub(crate) use files_compressed_gz::GzipFileIsNotMultiArchSameSafe;
pub(crate) use files_compressed_gz::PackageContainsTimestampedGzip;
pub(crate) use files_compressed_lzma::BrokenLzma;
pub(crate) use files_compressed_xz::BrokenXz;
pub(crate) use files_duplicates::DuplicateChangelogFiles;
pub(crate) use files_init::NonStandardFilePermissionsForEtcInitDScript;
pub(crate) use files_init::PackageInstallsDeprecatedUpstartConfiguration;
pub(crate) use files_init::PackageInstallsIntoEtcRcBoot;
pub(crate) use files_init::PackageInstallsIntoEtcRcD;
pub(crate) use files_ld_so::PackageModifiesLdSoSearchPath;
pub(crate) use files_missing::MissingIntermediateDirectory;
pub(crate) use files_scripts::ScriptWithLanguageExtension;
pub(crate) use files_special::SpecialFile;
pub(crate) use fonts::DuplicateFontFile;
pub(crate) use fonts::FontInNonFontPackage;
pub(crate) use fonts::WoffFontInWrongDir;
pub(crate) use images_thumbnails::PackageContainsThumbnailsDir;
pub(crate) use images_thumbnails::PackageContainsXvpicsDir;
pub(crate) use includes_config_h::PackageNameDefinedInConfigH;
pub(crate) use languages_fortran_gfortran::GfortranModuleDoesNotDeclareVersion;
pub(crate) use languages_fortran_gfortran::MissingPrerequisiteForGfortranModule;
pub(crate) use languages_golang_module_path::GoLibraryPackageDoesNotIncludeVersion;
pub(crate) use languages_java::BadJarName;
pub(crate) use languages_java::ClasspathContainsRelativePath;
pub(crate) use languages_java::CodelessJar;
pub(crate) use languages_java::ExecutableJarWithoutMainClass;
pub(crate) use languages_java::IncompatibleJavaBytecodeFormat;
pub(crate) use languages_java::JarContainsSource;
pub(crate) use languages_java::JarNotInUsrShare;
pub(crate) use languages_java::MavenPluginInUsrShareJava;
pub(crate) use languages_java::MissingDepOnJarwrapper;
pub(crate) use languages_java::MissingManifest;
pub(crate) use languages_java::UnknownJavaClassVersion;
pub(crate) use languages_java::ZipParseError;
pub(crate) use languages_java_bytecode::PackageInstallsJavaBytecode;
pub(crate) use languages_puppet::PuppetModuleButNotDhPuppet;
pub(crate) use languages_ruby::RubyInterpreterIsDeprecated;
pub(crate) use languages_ruby::RubygemHomepage;
pub(crate) use languages_rust::EmptyRustLibraryDeclaresProvides;
pub(crate) use libraries_debug_symbols::StrippedLibrary;
pub(crate) use libraries_shared_file_permissions::OddPermissionsOnSharedLibrary;
pub(crate) use libraries_shared_file_permissions::SharedLibraryIsExecutable;
pub(crate) use libraries_shared_multi_arch::SharedLibraryIsMultiArchForeign;
pub(crate) use libraries_shared_relocation::SpecificAddressInSharedLibrary;
pub(crate) use libraries_shared_stack::ExecutableStackInSharedLibrary;
pub(crate) use libraries_shared_stack::SharedLibraryLacksStackSection;
pub(crate) use maintainer_scripts_dpkg_statoverride::UnconditionalUseOfDpkgStatoverride;
pub(crate) use origtar::EmptyUpstreamSources;
pub(crate) use shell_csh::CshConsideredHarmful;
pub(crate) use substvars_libc::PackageDependsOnHardcodedLibc;
pub(crate) use substvars_misc_pre_depends::DependsOnMiscPreDepends;
pub(crate) use template_dh_make_control::ControlFileContainsDhMakeVcsComment;
pub(crate) use usrmerge::{FileInRootAndUsr, LibraryInRootAndUsr};

pub(crate) use debian_changelog::{
    BogusMailHostInDebianChangelog, ChangelogEmptyEntry, ChangelogFileMissingExplicitEntry,
    ChangelogIsDhMakeTemplate, ChangelogIsSymlink, ChangelogNewsDebianMismatch,
    ChangelogNotCompressedWithMaxCompression, ChangelogReferencesTempSecurityIdentifier,
    DebianChangelogFileContainsObsoleteUserEmacsSettings, DebianChangelogHasWrongDayOfWeek,
    DebianChangelogLineTooLong, DebianNewsEntryHasStrangeDistribution,
    DebianNewsEntryHasUnknownVersion, DebianNewsFileNotCompressed, EpochChangeWithoutComment,
    EpochChangedButUpstreamVersionDidNotGoBackwards, ExtraWhitespaceAroundNameInChangelogTrailer,
    HtmlChangelogWithoutTextVersion, ImprobableBugNumberInCloses, InitialUploadClosesNoBugs,
    InvalidDateInDebianChangelog, LatestChangelogEntryWithoutNewDate,
    LatestDebianChangelogEntryReusesExistingVersion, MaintainerUploadHasIncorrectVersionNumber,
    MalformedDebianChangelogVersion, MisspelledClosesBug, NewPackageUsesDateBasedVersionNumber,
    NmuInChangelog, NoNmuInChangelog, NoQaInChangelog, OddHistoricalDebianChangelogVersion,
    PossibleNewUpstreamReleaseWithoutNewVersion, QaUploadHasIncorrectVersionNumber,
    RcVersionGreaterThanExpectedVersion, SourceNmuHasIncorrectVersionNumber,
    SyntaxErrorInDebianNewsFile, TeamUploadHasIncorrectVersionNumber, UnnecessaryTeamUpload,
    UploadersInOrphan, VersionRefersToDistribution,
    VersionRefersToUnstableDistributionWithExpVersion, WrongBugNumberInCloses,
    WrongNameForDebianNewsFile,
};
pub(crate) use debian_copyright::{
    AmbiguousParagraphInDep5Copyright, BadExceptionFormatInDep5Copyright,
    BoilerplateCopyrightFormatUri, CommaSeparatedFilesInDep5Copyright,
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
    Dep5FileParagraphReferencesHeaderParagraph, DuplicateGlobbingPatterns,
    EmptyShortLicenseInDep5Copyright, FileIncludedAlready, FileWithoutCopyrightInformation,
    FilesExcludedWithoutCopyrightFormat10, GlobalFilesWildcardNotFirstParagraphInDep5Copyright,
    GlobbingPatternsOutOfOrder, HelperTemplatesInCopyright, IncompleteCreativeCommonsLicense,
    InvalidEscapeSequenceInDep5Copyright, InvalidShortNameInDep5Copyright,
    LicenseProblemUndefinedLicense, MissingFieldInDep5Copyright,
    MissingLicenseParagraphInDep5Copyright, MissingLicenseTextInDep5Copyright,
    NamedCopyrightForSingleInstallable, NoCopyrightFile, NoDebianCopyrightInSource,
    NoDep5Copyright, ObsoleteFieldInDep5Copyright, OldFsfAddressInCopyrightFile,
    PipeSymbolUsedAsLicenseDisjunction, SourceShipsExcludedFile,
    SpaceInStdShortnameInDep5Copyright, SyntaxErrorInDep5Copyright, TabInLicenseText,
    UnknownParagraphInDep5Copyright, UnusedLicenseParagraphInDep5Copyright,
    UsrShareDocSymlinkWithoutDependency,
};
pub(crate) use fields_version::{
    BadVersionNumber, BinaryNmuDebianRevisionInSource, DebianRevisionIsZero,
    DebianRevisionNotWellFormed, DfsgVersionInNativePackage, DfsgVersionMisspelled,
};

pub(crate) use debian_debconf::{
    BooleanTemplateHasBogusDefault, ConfigDoesNotLoadConfmodule, DebconfConfigNotExecutable,
    DebconfIsNotARegistry, DuplicateLongDescriptionInTemplate, EmptyTranslatedChoices,
    IsdefaultFlagIsDeprecated, LoadsObsoleteConfmodule,
    MakingAssumptionsAboutInterfacesInTemplates, MalformedPromptInTemplates,
    MalformedQuestionInTemplates, MalformedTemplateName, MalformedTitleInTemplates,
    MismatchTranslatedChoices, MissingDebconfDependency, MissingDebconfDependencyForPreinst,
    NoDebconfTemplates, NoTemplateDescription, NoTemplateName, NoTemplateType,
    PossibleDebconfNoteAbuse, PostinstDoesNotLoadConfmodule, PostinstUsesDbInput,
    PostrmDoesNotPurgeDebconf, SelectWithBooleanChoices, SelectWithoutChoices,
    SyntaxErrorInDebconfTemplate, TemplateUsesUnsplitChoices,
    TooLongExtendedDescriptionInTemplates, TooLongShortDescriptionInTemplates,
    UnknownDebconfPriority, UnknownFieldInTemplates, UnknownTemplateType,
    UsingFirstPersonInTemplates, UsingImperativeFormInTemplates,
    UsingQuestionInExtendedDescriptionInTemplates,
};
pub(crate) use debian_filenames::{DuplicatePackagingFile, IncorrectPackagingFilename};
pub(crate) use debian_line_separators::CarriageReturnLineFeed;
pub(crate) use debian_readme::{
    ReadmeDebianContainsDebmakeTemplate, ReadmeDebianContainsInvalidEmailAddress,
    ReadmeDebianMentionsUsrDoc,
};
pub(crate) use debian_rules::{
    BinaryArchRulesButPkgIsArchIndep, CustomCompressionInDebianRules,
    DebianRulesCallsDebhelperInOddOrder, DebianRulesCallsNproc, DebianRulesCallsPwd,
    DebianRulesIgnoresMakeCleanError, DebianRulesIsDhMakeTemplate, DebianRulesIsSymlink,
    DebianRulesMissingRequiredTarget, DebianRulesNotAMakefile,
    DebianRulesPassesVersionInfoToDhShlibdeps, DebianRulesSetsDebBuildOptions,
    DebianRulesSetsDhCompat, DebianRulesSetsDpkgArchitectureVariable,
    DebianRulesShouldNotSetCFLAGSFromNoopt, DebianRulesShouldNotUseDhExtraAddons,
    DebianRulesShouldNotUseSanitizeAllBuildflag, DebianRulesUpdatesControlAutomatically,
    DebianRulesUsesDebBuildOpts, DebianRulesUsesInstalledPythonVersions,
    DebianRulesUsesSpecialShellVariable,
    DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
    DebianRulesUsesWrongEnvironmentVariable, MissingBuildDependsForCleanTargetInDebianRules,
    OverrideDhCleanDoesNotCallDhClean, OverrideDhFixpermsDoesNotCallDhFixperms,
    RulesRequireBuildPrerequisite,
};
pub(crate) use debian_shlibs::{
    DistantPrerequisiteInShlibs, DuplicateEntryInSymbolsControlFile, DuplicateInShlibs,
    EmptyShlibs, InvalidTemplateIdInSymbolsFile, NoShlibs, OutdatedRelationInShlibs,
    SharedLibraryLacksVersion, SharedLibraryNotShipped, SharedLibrarySymbolsNotTracked,
    ShipsUndeclaredSharedLibrary, SurplusSharedLibrarySymbols,
    SymbolsFileContainsCurrentVersionWithDebianRevision, SymbolsFileContainsDebianRevision,
    SymbolsForUndeclaredSharedLibrary, SyntaxErrorInSymbolsFile, UnknownMetaFieldInSymbolsFile,
};
pub(crate) use debian_version_substvars::{
    NotBinnmuableAllDependsAny, NotBinnmuableAnyDependsAll, NotBinnmuableAnyDependsAny,
    VersionSubstvarForExternalPackage,
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
    DescriptionContainsTabs, DescriptionIsDebmakeTemplate, DescriptionIsDhMakeTemplate,
    DescriptionIsPkgName, DescriptionStartsWithLeadingSpaces, DescriptionStartsWithPackageName,
    DescriptionSynopsisIsEmpty, DescriptionSynopsisStartsWithArticle, DescriptionTooShort,
    ExtendedDescriptionContainsEmptyParagraph, ExtendedDescriptionIsEmpty,
    ExtendedDescriptionLineTooLong, SynopsisTooLong,
};
pub(crate) use documentation_examples::NestedExamplesDirectory;
pub(crate) use documentation_manual::{
    BadSoLinkWithinManualPage, BadWhatisEntry, BrokenLinkToUndocumented, BuildPathInManual,
    CountryInManual, EmptyManualPage, ExecutableManualPage, GroffMessage, ManualPageFromTemplate,
    ManualPageWithGenericName, NoEnglishManualPage, NoManualPage, OddPlaceForManualPage,
    PodConversionMessage, PoorCompressionInManualPage, UncompressedManualPage,
    UndocumentedManualPage, UselessWhatisEntry, WrongCompressionInManualPage, WrongManualSection,
    WrongNameForManualPage,
};
pub(crate) use documentation_texinfo::{
    InfoDocumentHasWrongExtension, InfoDocumentMissingDirEntry, InfoDocumentMissingDirSection,
    InfoDocumentMissingImageFile, InfoDocumentNotCompressed, InfoDocumentNotCompressedWithGzip,
    InfoDocumentNotCompressedWithMaxCompression,
};
pub(crate) use executable::ExecutableNotElfOrScript;
pub(crate) use fields_bugs::BugsFieldDoesNotReferToDebianInfrastructure;
pub(crate) use fields_package_relations::{
    BuildDependsOn1Revision, BuildDependsOnBuildEssential, BuildDependsOnMetapackage,
    BuildDependsOnObsoletePackage, DependsExclusivelyOnMakedev, DependsOnObsoletePackage,
    DependsOnOldEmacs, NeedlessSuggestRecommendLibservletJava, PackageRelationWithSelf,
    VirtualPackageDependsWithoutRealPackageDepends,
};
pub(crate) use fields_relations_more::{
    AlternatesNotAllowed, BadProvidedPackageName, BadRelation, BuildConflictsWithBuildDependency,
    BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
    BuildDependsOnEssentialPackageWithoutUsingVersion, BuildDependsOnObsoleteBootstrap,
    ConflictsWithDependency, DependsOnEssentialPackageWithoutUsingVersion, DependsOnMetapackage,
    DependsOnObsoleteBootstrap, DependsOnPackagingDev, DependsOnPythonMinimal,
    InvalidArchStringInSourceRelation, InvalidProfileNameInSourceRelation,
    LibRecommendsDocumentation, LibmoduleBuildPerlNeedsToBeInBuildDepends,
    LibmoduleBuildTinyPerlNeedsToBeInBuildDepends,
    MailTransportAgentDependencyDoesNotSpecifyDefaultMta, NeedlessDependencyOnJre,
    NeedlesslyDependsOnAwk, ObsoleteRelationForm, PackageDependsOnMultipleLibpngVersions,
    PackageDependsOnMultipleLibstdcVersions, PackageDependsOnMultipleTclVersions,
    PackageDependsOnMultipleTkVersions, PackageRelationWithPerlModules,
    RedundantBuildPrerequisites,
};
pub(crate) use fields_text::{
    BadHomepage, DescriptionContainsHomepage, DescriptionSynopsisIsDuplicated,
    EssentialInSourcePackage, EssentialNoNotNeeded, ExcessivePriorityForLibraryPackage,
    ExplicitDefaultInPackageType, FieldTooLong, HomepageForCpanPackageContainsVersion,
    HomepageRefersToObsoleteDebianInfrastructure, MissingSeparatorBetweenItems,
    NewEssentialPackage, PriorityExtraIsReplacedByPriorityOptional, RecommendedField,
    RedundantBugsField, RedundantOriginField, SectionIsDhMakeTemplate,
    SuperfluousClutterInHomepage, UnknownEssentialValue, UnknownPriority, UnknownSection,
    WrongSectionForUdeb,
};
pub(crate) use fields_uploaders::MaintainerAlsoInUploaders;
pub(crate) use fields_uploaders::UploaderNameMissing;
pub(crate) use fields_vcs::{
    OrphanedPackageMaintainedInPrivateSpace, OrphanedPackageNotMaintainedInDebianInfrastructure,
    VcsFieldBitrotted, VcsFieldMismatch,
};
pub(crate) use files_artifact::{
    DebianAddsArchControlDir, DebianAddsArchInventoryFile, DebianAddsBtsControlDir,
    DebianAddsBzrControlDir, DebianAddsCvsConflictCopy, DebianAddsCvsControlDir,
    DebianAddsEditorBackupFile, DebianAddsGitControlDir, DebianAddsHgControlDir,
    DebianAddsHgTagsFile, DebianAddsPatchFailureFile, DebianAddsQuiltControlDir,
    DebianAddsSvkCommitFile, DebianAddsSvnCommitFile, DebianAddsSvnConflictFile,
    DebianAddsSvnControlDir, NoDebianChanges,
};
pub(crate) use files_build_path::DirOrFileInBuildTree;
pub(crate) use files_config_scripts::{
    OldStyleConfigScriptMultiarchPath, OldStyleConfigScriptMultiarchPathArchAll,
};
pub(crate) use files_debug::NonDebugFileInDebugPackage;
pub(crate) use files_foreign::{
    CompressedDocumentation, DebugSuffixNotDbg, FileDirectlyInUsrShareDoc,
    MacosDsStoreFileInPackage, MacosResourceForkFileInPackage, PackageContainsInfoDirFile,
    PackageContainsLindaOverride, PackageContainsReadmeForOtherPlatformOrDistro,
    PackageInstallsAppimage, WindowsDevelFileInPackage, WindowsThumbnailDatabaseInPackage,
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
    BadPermissionsForEtcCronDScript, FileInUsrLibSgml, PackageInstallsIntoEtcGconfSchemas,
    RunPartsCronFilenameContainsIllegalChars,
};
pub(crate) use files_multi_arch::{
    MultiarchForeignCmakeFile, MultiarchForeignPkgconfig, MultiarchForeignSharedLibrary,
    MultiarchForeignStaticLibrary,
};
pub(crate) use files_names::{
    FileNameContainsWildcardCharacter, FileNameEndsInWhitespace, FileNameInPathIsNotAscii,
    HyphenFile, NativeSourceFileWithoutUtf8Name, PackageContainsCompiledGlibSchema,
    PackageContainsEslintConfigFile, PackageContainsFileInEtcSkel,
    PackageContainsFileInUsrShareHal, PackageContainsIconCacheInGenericDir,
    PackageContainsNpmIgnoreFile, PackageContainsPythonCoverageFile,
    PackageContainsPythonDoctreeFile, PackageContainsPythonDotDirectory,
    PackageContainsPythonHeaderInIncorrectDirectory, PackageContainsPythonHypothesisExample,
    PackageContainsPythonTestsInGlobalNamespace, PackageContainsSassCacheDirectory,
    ShippedFileWithoutUtf8Name, StarFile, ZeroByteExecutableInPath,
};
pub(crate) use files_obsolete_paths::PackageInstallsIntoObsoleteDir;
pub(crate) use files_p11_kit::IncorrectNamingOfPkcs11Module;
pub(crate) use files_pam::ConfigFileReserved;
pub(crate) use files_permissions::BadPermForFileInEtcSudoersD;
pub(crate) use files_permissions::BadPermissionsForAliFile;
pub(crate) use files_permissions::ElevatedPrivileges;
pub(crate) use files_permissions::ExecutableIsNotWorldReadable;
pub(crate) use files_permissions::NonStandardDirPerm;
pub(crate) use files_permissions::NonStandardExecutablePerm;
pub(crate) use files_permissions::NonStandardFilePerm;
pub(crate) use files_permissions::NonStandardSetuidExecutablePerm;
pub(crate) use files_pkgconfig::{
    PkgConfigBadDirective, PkgConfigMultiArchWrongDir, PkgConfigUnavailableForCrossCompilation,
};
pub(crate) use files_symbolic_links::{
    AbsoluteSymbolicLinkTargetInSource, AbsoluteSymlinkInTopLevelFolder,
    CompressedSymlinkWithWrongExt, LengthySymlink, PackageContainsUnsafeSymlink, RelativeSymlink,
    SymlinkContainsSpuriousSegments, SymlinkEndsWithSlash, SymlinkHasDoubleSlash,
    SymlinkHasTooManyUpSegments, SymlinkIsSelfRecursive, WaywardSymbolicLinkTargetInSource,
};
pub(crate) use files_unwanted::{BackupFileInPackage, NfsTemporaryFileInPackage};
pub(crate) use files_vcs::{
    PackageContainsVcsControlDir, PackageContainsVcsControlFile, SvkCommitFileInPackage,
    SvnCommitFileInPackage,
};
pub(crate) use fonts_opentype::{
    OpentypeFontProhibitsInstallableEmbedding, OpentypeFontWrongFilename,
};
pub(crate) use fonts_truetype::TruetypeFontProhibitsInstallableEmbedding;
pub(crate) use fonts_truetype::TruetypeFontWrongFilename;
pub(crate) use games::{
    GameOutsideSection, GlobalDataInGamesDirectory, PackageSectionGamesButContainsNoGame,
    PackageSectionGamesButHasUsrBin,
};
pub(crate) use init_scripts::{
    DirectoryInEtcSvDirectoryWithoutExecutableRunScript, DuplicateUpdatercDCallsInPostinst,
    DuplicateUpdatercDCallsInPostrm, InitDScriptContainsSkeletonTemplateContent,
    InitDScriptDependsOnAllVirtualFacility, InitDScriptDependsOnUnknownVirtualFacility,
    InitDScriptDoesNotImplementRequiredOption, InitDScriptHasBadLsbLine,
    InitDScriptHasBadStartRunlevel, InitDScriptHasBadStopRunlevel,
    InitDScriptHasConflictingStartStop, InitDScriptHasDuplicateLsbKeyword,
    InitDScriptHasDuplicateLsbSection, InitDScriptHasUnknownLsbKeyword,
    InitDScriptHasUnterminatedLsbSection, InitDScriptMissingDependencyOnLocalFs,
    InitDScriptMissingLsbKeyword, InitDScriptMissingLsbSection, InitDScriptMissingStart,
    InitDScriptNotIncludedInPackage, InitDScriptNotMarkedAsConffile,
    InitDScriptPossibleMissingStop, InitDScriptProvidesVirtualFacility,
    InitDScriptShouldAlwaysStartService, InitDScriptSourcingWithoutTest,
    InitDScriptStartsInStopRunlevel, InitDScriptStopsInSRunlevel, InitDScriptUsesUsrInterpreter,
    MissingSystemdServiceForInitDRcSScript, NonVirtualFacilityInInitdScript,
    PostrmContainsAdditionalUpdatercDCalls, PostrmDoesNotCallUpdatercDForInitDScript,
    PreinstCallsUpdatercD, PrermCallsUpdatercD, ScriptInEtcInitDNotRegisteredViaUpdateRcD,
    SystemdServiceFileRefersToObsoleteTarget, SystemdServiceFileWrapsInitScript,
    UpstartJobInEtcInitDNotRegisteredViaUpdateRcD,
};
pub(crate) use languages_javascript::{
    EmbeddedJavascriptLibrary, NodePackageInstallInNodejsRootdir, NodejsLockFile,
    NodejsModuleInstalledInBadDirectory, NodejsModuleInstalledInUsrLib,
    PkgJsAutopkgtestFileDoesNotExist, PkgJsToolsTestIsEmpty, PkgJsToolsTestIsMissing,
};
pub(crate) use languages_ocaml::{OcamlDanglingCmxa, OcamlDanglingCmxs};
pub(crate) use languages_perl::{
    PackageInstallsNonbinaryPerlInUsrLibPerl5, PackageInstallsPacklist,
    PackageInstallsPerllocalPod, PerlModuleInCoreDirectory, PerlModuleUsesPerl4LibsWithoutDep,
    ScriptUsesPerl4LibsWithoutDep,
};
pub(crate) use languages_python::{
    AncientPythonVersionField, BogusPythonPrerequisite, DjangoPackageDoesNotDependOnDjango,
    DuplicateP3distOverride, EmbeddedFeedparserLibrary, FileInUsrLibSitePython,
    MalformedPythonVersion, NewPackageShouldNotPackagePython2Module,
    PackageInstallsDeprecatedPython2Path, PackageInstallsPythonBytecode, PackageInstallsPythonEgg,
    PackageInstallsPythonPycacheDir, PypiHomepage, PythonDebugInWrongLocation,
    PythonModuleHasOverlyGenericName, PythonModuleInWrongLocation,
    PythonPackageMissingDependsOnPython, PythonVersionCurrentIsDeprecated,
    ScriptUsesUnversionedPythonInShebang, SourcePackageEncodesPythonVersion,
    ThirdPartyPackageInPythonDir, UnknownFileInPythonModuleDirectory,
};
pub(crate) use libraries_shared_soname::PackageNameDoesntMatchSonames;
pub(crate) use libraries_shared_soname::SharedobjectInLibraryDirectoryMissingSoname;
pub(crate) use libraries_static::OddStaticLibraryName;
pub(crate) use libtool::{IncorrectLibdirInLaFile, NonEmptyDependencyLibsInLaFile};
pub(crate) use mailcap::QuotedPlaceholderInMailcapEntry;
pub(crate) use md5sums::{
    FileMissingInMd5sums, MalformedMd5sumsControlFile, Md5sumMismatch, Md5sumsListsNonexistentFile,
};

pub(crate) use debian_maintscript::MaintscriptIncludesMaintScriptParameters;
pub(crate) use debian_patches::{
    Format30ButDebianChangesPatch, InvalidDep3FormatPatchBugFieldDefaultValue,
    InvalidDep3FormatPatchBugNotContainUrl, InvalidDep3FormatPatchMaybeBugDebian,
    InvalidDep3FormatPatchOriginFieldDefaultValue, PatchFilePresentButNotMentionedInSeries,
    PatchSystemButNoSourceReadme, QuiltBuildDepButNoSeriesFile,
};
pub(crate) use debian_po_debconf::{
    StrayTranslatedDebconfTemplates, UntranslatableDebconfTemplates,
};
pub(crate) use debian_source_dir::{
    CustomCompressionInDebianSourceOptions, GitPatchesNotExported, MissingDebianSourceFormat,
};
pub(crate) use debian_variables::IllegalVariable;
pub(crate) use emacs::BadPermissionsForEtcEmacsScript;
pub(crate) use emacs_elpa::EmacsenCommonWithoutDhElpa;
pub(crate) use fields_architecture::{
    ArchWildcardInBinaryPackage, AspellPackageNotArchAll,
    DocumentationPackageNotArchitectureIndependent, MagicArchInArchList, TooManyArchitectures,
    UnknownArchitecture,
};
pub(crate) use fields_built_using::InvalidValueInBuiltUsingField;
pub(crate) use fields_installer_menu_item::BadMenuItem;
pub(crate) use fields_mail_address::{
    DuplicateContact, FaultyDebianQaGroupAddress, FaultyDebianQaGroupPhrase,
    MailAddressLoopsOrBounces, RootInContact,
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
pub(crate) use fields_standards_version::{
    InvalidStandardsVersion, NewerStandardsVersion, TimewarpStandardsVersion,
};
pub(crate) use fields_static_built_using::InvalidValueInStaticBuiltUsingField;
pub(crate) use files_non_free::LicenseProblemMd5sumNonFreeFile;
pub(crate) use files_non_free::NonFreeFlash;
pub(crate) use files_privacy_breach::{
    PrivacyBreachDonation, PrivacyBreachFacebook, PrivacyBreachGeneric, PrivacyBreachGoogleAdsense,
    PrivacyBreachGoogleCse, PrivacyBreachGooglePlus, PrivacyBreachLogo, PrivacyBreachPiwik,
    PrivacyBreachStatisticsWebsite, PrivacyBreachTwitter, PrivacyBreachUsesEmbeddedFile,
    PrivacyBreachW3cValidHtml,
};
pub(crate) use languages_php::{
    ComposerPrerequisite, EmbeddedPhpLibrary, ObsoleteCommentsStyleInPhpIni,
};
pub(crate) use languages_r::RDataWithoutReadmeSource;
pub(crate) use languages_r::RPackageNotArchAll;
pub(crate) use lintian_overrides::{
    AlienTag, DuplicateOverrideContext, MalformedOverride, OverrideFileInWrongLocation,
    OverrideFileInWrongPackage,
};
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
pub(crate) use ln0858_package_contains_cmake_private_file::PackageContainsCmakePrivateFile;
pub(crate) use ln0927_missing_depends_on_sensible_utils::MissingDependsOnSensibleUtils;
pub(crate) use ln1086_source_contains_prebuilt_silverlight_object::SourceContainsPrebuiltSilverlightObject;
pub(crate) use maintainer_scripts_adduser::AdduserWithHomeVarRun;
pub(crate) use maintainer_scripts_diversion::{
    DiversionForUnknownFile, OrphanedDiversion, PackageUsesLocalDiversion,
    RemoveOfUnknownDiversion, SystemdDiversion,
};
pub(crate) use maintainer_scripts_killall::KillallIsDangerous;
pub(crate) use maintainer_scripts_mknod::MknodInMaintainerScript;
pub(crate) use maintainer_scripts_systemctl::MaintainerScriptCallsSystemctl;
pub(crate) use menu_format::{
    BadTestInMenuItem, CommandInMenuFileAndDesktopFile, DesktopCommandNotInPackage,
    DesktopContainsDeprecatedKey, DesktopEntryContainsDeprecatedKey,
    DesktopEntryContainsUnknownKey, DesktopEntryFileHasCrs, DesktopEntryInvalidCategory,
    DesktopEntryLacksMainCategory, DesktopEntryMissingRequiredKey, DesktopEntryUnknownType,
    DesktopEntryUsesReservedCategory, DesktopMimeButNoExecCode, DuplicateKeyInDesktop,
    DuplicateTagInMenu, ExecutableDesktopFile, MenuCommandNotInPackage, MenuIconCannotBeParsed,
    MenuIconMissing, MenuIconNotInXpmFormat, MenuIconTooBig, MenuIconUsesRelativePath,
    MenuItemContainsUnknownTag, MenuItemCreatesNewRootSection, MenuItemCreatesNewSection,
    MenuItemMissingRequiredTag, MenuItemNeedsDwww, MenuItemNeedsTagHasUnknownValue,
    MenuItemUsesAppsGamesSection, MenuItemUsesAppsSection, MenuItemUsesIconNone,
    MenuItemUsesWindowmanagersSection, NonWmInWindowmanagerMenuSection,
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
    DuplicateFormatInDocBase, ExecutableInUsrShareDocbase, ExecutableMenuFile,
    MaintainerScriptDoesNotCheckForExistenceOfInstalldocs,
    MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus, MenuMethodLacksInclude,
    PostinstDoesNotCallUpdatemenus, PostinstHasUselessCallToInstallDocs,
    PostrmDoesNotCallUpdatemenus, PrermHasUselessCallToInstallDocs,
};
pub(crate) use mimeinfo::PackageContainsMimeCacheFile;
pub(crate) use mimeinfo::PackageContainsMimeFileOutsidePackageDir;
pub(crate) use mimeinfo::PackageContainsMimeinfoCacheFile;
pub(crate) use modprobe::{
    DirectoryInModprobeD, NonConfFileInModprobeD, ObsoleteCommandInModprobeDFile,
};
pub(crate) use nodejs::{NodejsMissingVersionOverride, NodejsModuleNotDeclared};
pub(crate) use po_debconf::{
    DebconfTranslationUsingGeneralList, InvalidPoFile, MisnamedPoFile, MissingFileFromPotfilesIn,
    MissingPotfilesIn, MissingTemplatesPot, NotUsingPoDebconf, TranslatedDefaultField,
    UnknownEncodingInPoFile,
};
pub(crate) use rules_and_control::{
    BraceExpansionInDebhelperConfigFile, BuildPrerequisiteInInstallableSection,
    CircularInstallationPrerequisite, DebhelperCompatFileContainsMultipleLevels,
    DebhelperCompatFileIsEmpty, DebhelperToolsFromAutotoolsDevAreDeprecated,
    DebianControlHasDbgsymPackage, DebianControlRepeatsFieldNameInValue,
    DebianRulesUsesDeprecatedSystemdOverride, DebianRulesUsesUnnecessaryDhArgument,
    DeclaresPossiblyConflictingDebhelperCompatVersions, DhCleanKIsDeprecated,
    DhExecInstallNotAllowedHere, DhExecPrivateHelper, DhExecScriptWithoutDhExecFeatures,
    DhInstallmanpagesIsObsolete, DhQuiltAddonButQuiltSourceFormat, ExcessiveDebhelperOverrides,
    ExecutableDebhelperFileWithoutBeingExecutable, PackageFileIsExecutable,
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
    ControlInterpreterInUsrLocal, ControlInterpreterWithoutDepends,
    ControlInterpreterWithoutPredepends, ExampleScriptUsesDeprecatedNodejsLocation,
    ForbiddenConfigInterpreter, ForbiddenPostrmInterpreter, GawkScriptButNoGawkDep,
    IncorrectPathForInterpreter, InstallInfoUsedInMaintainerScript, InterpreterInUsrLocal,
    InterpreterNotAbsolute, MaintainerScriptCallsChownImproperly,
    MaintainerScriptCallsInstallSgmlcatalog, MaintainerScriptChangesLdSoConf,
    MaintainerScriptChangesNetbase, MaintainerScriptHasInvalidUpdateInetdOptions,
    MaintainerScriptHasUnexpandedDebhelperToken, MaintainerScriptHidesInitFailure,
    MaintainerScriptModifiesInetdConf, MaintainerScriptSetsAlternativeImproperly,
    MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
    MaintainerScriptShouldNotUsePiupartsVariable, MaintainerScriptUpdatesFontconfigCacheImproperly,
    MaintscriptCallsLdconfig, MawkScriptButNoMawkDep, MissingCallToDpkgMaintscriptHelper,
    MultiArchSamePackageCallsPycompile, PhpScriptWithUnusualInterpreter,
    PossiblyInsecureHandlingOfTmpFilesInMaintainerScript, PostrmRemovesAlternative,
    Python3ScriptButNoPython3Dep, ScriptUsesBinEnv, SystemdAlternatives, UdebPostinstCallsLdconfig,
    UdevadmCalledWithoutGuard, UnusualControlInterpreter,
};
pub(crate) use systemd::{
    InitDScriptDoesNotSourceInitFunctions, InitScriptIsNotAFile, KillModeNone,
    MisplacedSystemdTmpfiles, MissingSystemdServiceForInitDScript,
    MissingSystemdTimerForCronScript, OmittedSystemdServiceForInitDScript,
    SystemdServiceAliasWithoutExtension, SystemdServiceFileRefersToObsoleteBindto,
    SystemdServiceFileRefersToUnusualWantedbyTarget,
    SystemdServiceFileUsesDeprecatedSyslogFacility, SystemdServiceFileUsesNobodyOrNogroup,
    SystemdServiceInOddLocation,
};
pub(crate) use testsuite::{
    BackgroundedTestCommand, ConflictingTestFields, DebianTestsControlAutodep8IsObsolete,
    DeclarePythonVersionsForTest, DropPythonVersionDeclaration, EmptyDebianTestsControl,
    IllegalRuntimeTestName, MissingRuntimeTestFile, MissingTestsControl, NoOpTestsuite,
    ObsoleteRuntimeTestsRestriction, RuntimeTestFileUsesInstalledPythonVersions,
    RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends, TestLeavesPythonVersionUntested,
    TestsuiteDependencyHasUnparsableElements, TestsuiteDependsOnObsoletePackage,
    TestsuiteRestrictionsHasDeprecatedSkipNotInstallable, UnknownTestsuite,
    UnnecessaryTestsuiteAutopkgtestField, UsesDeprecatedAdttmp,
};
pub(crate) use triggers::UsesImplicitAwaitTrigger;
pub(crate) use vim::VimAddonWithinVimRuntimePath;
