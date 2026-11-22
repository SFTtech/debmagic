use super::rule::{BinaryPackageRule, RuleAccess, SourceRule};
use super::rules::AlienTag;
use super::rules::AlternatesNotAllowed;
use super::rules::Apache2DeprecatedAuthConfig;
use super::rules::Apache2ModuleDoesNotDependOnApache2Api;
use super::rules::Apache2ModuleDoesNotShipLoadFile;
use super::rules::Apache2ReverseDependencyCallsInvokeRcD;
use super::rules::Apache2ReverseDependencyShipsFileInNotAllowedDirectory;
use super::rules::Apache2ReverseDependencyUsesObsoleteDirectory;
use super::rules::Apache2UnsupportedDependency;
use super::rules::ArchIndependentPackageContainsBinaryOrObject;
use super::rules::ArchWildcardInBinaryPackage;
use super::rules::BadJarName;
use super::rules::BadPackageName;
use super::rules::BadPermForFileInEtcSudoersD;
use super::rules::BadPermissionsForAliFile;
use super::rules::BadPermissionsForEtcCronDScript;
use super::rules::BadRelation;
use super::rules::BadVersionInRelation;
use super::rules::BadVersionNumber;
use super::rules::BinaryCompiledWithProfilingEnabled;
use super::rules::BinaryInEtc;
use super::rules::BinaryNmuDebianRevisionInSource;
use super::rules::BinaryPackageDependsOnToolchainPackage;
use super::rules::BinaryWithBadDynamicTable;
use super::rules::BoilerplateCopyrightFormatUri;
use super::rules::BreaksWithoutVersion;
use super::rules::BrokenGz;
use super::rules::BrokenLzma;
use super::rules::BrokenXz;
use super::rules::BrokenZip;
use super::rules::BuildDependsArchWithoutArchDependentBinary;
use super::rules::BuildDependsIndepWithoutArchIndep;
use super::rules::BuildDependsOnAnObsoleteJavaPackage;
use super::rules::BuildDependsOnDhSysuser;
use super::rules::BuildDependsOnNonBuildPackage;
use super::rules::BuildDependsOnSpecificJavaDocPackage;
use super::rules::BuildDependsOnVersionedBerkeleyDb;
use super::rules::CarriageReturnLineFeed;
use super::rules::ChangelogFileMissingExplicitEntry;
use super::rules::ChangelogIsDhMakeTemplate;
use super::rules::ChangelogIsSymlink;
use super::rules::ChangelogNotCompressedWithMaxCompression;
use super::rules::ClasspathContainsRelativePath;
use super::rules::CodelessJar;
use super::rules::CommaSeparatedFilesInDep5Copyright;
use super::rules::ComposerPrerequisite;
use super::rules::ConfigDoesNotLoadConfmodule;
use super::rules::ConfigureGeneratedFileInSource;
use super::rules::ConflictingNegationInSourceRelation;
use super::rules::ControlFileContainsDhMakeVcsComment;
use super::rules::ControlFileHasBadOwner;
use super::rules::ControlFileHasBadPermissions;
use super::rules::ControlFileIsEmpty;
use super::rules::ControlFileIsNotAFile;
use super::rules::ControlInterpreterInUsrLocal;
use super::rules::ControlInterpreterWithoutDepends;
use super::rules::ControlInterpreterWithoutPredepends;
use super::rules::CshConsideredHarmful;
use super::rules::CustomLibrarySearchPath;
use super::rules::DbgPackageMissingDepends;
use super::rules::DbusPolicyAtConsole;
use super::rules::DbusPolicyExcessivelyBroad;
use super::rules::DbusPolicyInEtc;
use super::rules::DbusPolicyWithoutSendDestination;
use super::rules::DbusSystemServiceWrongName;
use super::rules::DebconfConfigNotExecutable;
use super::rules::DebconfIsNotARegistry;
use super::rules::DebconfTranslationUsingGeneralList;
use super::rules::DebhelperCompatFileContainsMultipleLevels;
use super::rules::DebhelperCompatFileIsEmpty;
use super::rules::DebianControlFileIsASymlink;
use super::rules::DebianRulesDefinesDpkgBuildApi;
use super::rules::DebianRulesIsDhMakeTemplate;
use super::rules::DebianRulesIsSymlink;
use super::rules::DebianRulesNotAMakefile;
use super::rules::DebianRulesPassesVersionInfoToDhShlibdeps;
use super::rules::DebianRulesSetsDebBuildOptions;
use super::rules::DebianRulesSetsDhCompat;
use super::rules::DebianRulesSetsDpkgArchitectureVariable;
use super::rules::DebianRulesShouldNotSetCFLAGSFromNoopt;
use super::rules::DebianRulesShouldNotUseDhExtraAddons;
use super::rules::DebianRulesShouldNotUseSanitizeAllBuildflag;
use super::rules::DebianRulesUpdatesControlAutomatically;
use super::rules::DebianRulesUsesDebBuildOpts;
use super::rules::DebianRulesUsesDeprecatedSystemdOverride;
use super::rules::DebianRulesUsesInstalledPythonVersions;
use super::rules::DebianRulesUsesSpecialShellVariable;
use super::rules::DebianRulesUsesUnnecessaryDhArgument;
use super::rules::DebianRulesUsesWrongEnvironmentVariable;
use super::rules::DebianUpstreamObsoletePath;
use super::rules::DebugSymbolsDirectlyInUsrLibDebug;
use super::rules::DebugSymbolsNotDetached;
use super::rules::DeclaresPossiblyConflictingDebhelperCompatVersions;
use super::rules::DefaultMtaDependencyDoesNotSpecifyMailTransportAgent;
use super::rules::DefaultMtaDependencyNotListedFirst;
use super::rules::DependsOnLibdb1Compat;
use super::rules::DependsOnMiscPreDepends;
use super::rules::DependsOnSpecificJavaDocPackage;
use super::rules::DescriptionIsDhMakeTemplate;
use super::rules::DhCleanKIsDeprecated;
use super::rules::DhExecInstallNotAllowedHere;
use super::rules::DhExecPrivateHelper;
use super::rules::DhExecScriptWithoutDhExecFeatures;
use super::rules::DhInstallmanpagesIsObsolete;
use super::rules::DhMakeTemplateInSource;
use super::rules::DhQuiltAddonButQuiltSourceFormat;
use super::rules::DirectoryInEtcSvDirectoryWithoutExecutableRunScript;
use super::rules::DistantPrerequisiteInShlibs;
use super::rules::DjangoPackageDoesNotDependOnDjango;
use super::rules::DocPackageDependsOnMainPackage;
use super::rules::DuplicateFontFile;
use super::rules::DuplicateGlobbingPatterns;
use super::rules::DuplicateOverrideContext;
use super::rules::DuplicateUpdatercDCallsInPostinst;
use super::rules::DuplicateUpdatercDCallsInPostrm;
use super::rules::ElevatedPrivileges;
use super::rules::EmptyField;
use super::rules::EmptyRustLibraryDeclaresProvides;
use super::rules::EmptyShlibs;
use super::rules::EmptyUpstreamSources;
use super::rules::ExampleScriptUsesDeprecatedNodejsLocation;
use super::rules::ExcessiveDebhelperOverrides;
use super::rules::ExecutableDebhelperFileWithoutBeingExecutable;
use super::rules::ExecutableDesktopFile;
use super::rules::ExecutableInUsrShareDocbase;
use super::rules::ExecutableIsNotWorldReadable;
use super::rules::ExecutableJarWithoutMainClass;
use super::rules::ExecutableManualPage;
use super::rules::ExecutableMenuFile;
use super::rules::ExecutableStackInSharedLibrary;
use super::rules::ExtraWhitespaceAroundNameInChangelogTrailer;
use super::rules::FileIncludedAlready;
use super::rules::FileWithoutCopyrightInformation;
use super::rules::FilesExcludedWithoutCopyrightFormat10;
use super::rules::FontInNonFontPackage;
use super::rules::FontPackageNotMultiArchForeign;
use super::rules::ForbiddenPostrmInterpreter;
use super::rules::GawkScriptButNoGawkDep;
use super::rules::GfortranModuleDoesNotDeclareVersion;
use super::rules::GirInArchAllPackage;
use super::rules::GirPackageNameDoesNotMatch;
use super::rules::GlobbingPatternsOutOfOrder;
use super::rules::GoLibraryPackageDoesNotIncludeVersion;
use super::rules::GzipFileIsNotMultiArchSameSafe;
use super::rules::HardeningNoBindnow;
use super::rules::HardeningNoFortifyFunctions;
use super::rules::HardeningNoPie;
use super::rules::HardeningNoRelro;
use super::rules::IllegalMultiArchValue;
use super::rules::IncompatibleJavaBytecodeFormat;
use super::rules::IncorrectPathForInterpreter;
use super::rules::InitDScriptContainsSkeletonTemplateContent;
use super::rules::InitDScriptDependsOnUnknownVirtualFacility;
use super::rules::InitDScriptUsesUsrInterpreter;
use super::rules::InitScriptIsNotAFile;
use super::rules::InterpreterInUsrLocal;
use super::rules::InterpreterNotAbsolute;
use super::rules::InvalidEscapeSequenceInDep5Copyright;
use super::rules::InvalidOverrideRestriction;
use super::rules::InvalidProfileNameInBuildProfilesField;
use super::rules::InvalidProfileNameInSourceRelation;
use super::rules::IsdefaultFlagIsDeprecated;
use super::rules::JarContainsSource;
use super::rules::JarNotInUsrShare;
use super::rules::KillModeNone;
use super::rules::LibRecommendsDocumentation;
use super::rules::LibappPerlPackageName;
use super::rules::LibmoduleBuildPerlNeedsToBeInBuildDepends;
use super::rules::LibmoduleBuildTinyPerlNeedsToBeInBuildDepends;
use super::rules::LicenseProblemMd5sumNonFreeFile;
use super::rules::MagicArchInArchList;
use super::rules::MaintainerScriptDoesNotCheckForExistenceOfInstalldocs;
use super::rules::MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus;
use super::rules::MaintainerScriptSetsAlternativeImproperly;
use super::rules::MaintainerUploadHasIncorrectVersionNumber;
use super::rules::MalformedDebianChangelogVersion;
use super::rules::MalformedOverride;
use super::rules::MalformedTemplateName;
use super::rules::MavenPluginInUsrShareJava;
use super::rules::MawkScriptButNoMawkDep;
use super::rules::MenuIconCannotBeParsed;
use super::rules::MenuIconNotInXpmFormat;
use super::rules::MenuIconTooBig;
use super::rules::MenuItemContainsUnknownTag;
use super::rules::MenuItemCreatesNewRootSection;
use super::rules::MenuItemNeedsDwww;
use super::rules::MenuItemUsesAppsGamesSection;
use super::rules::MenuItemUsesIconNone;
use super::rules::MisplacedExtraMemberInDeb;
use super::rules::MisplacedSystemdTmpfiles;
use super::rules::MissingBuildDependsForCleanTargetInDebianRules;
use super::rules::MissingCallToUpdateFonts;
use super::rules::MissingDebconfDependency;
use super::rules::MissingDepOnJarwrapper;
use super::rules::MissingDependencyOnPhpapi;
use super::rules::MissingDependsOnSensibleUtils;
use super::rules::MissingFileFromPotfilesIn;
use super::rules::MissingIntermediateDirectory;
use super::rules::MissingManifest;
use super::rules::MissingPotfilesIn;
use super::rules::MissingPrerequisiteForGfortranModule;
use super::rules::MissingTemplatesPot;
use super::rules::MissingTestsControl;
use super::rules::MisspelledClosesBug;
use super::rules::MultiArchSamePackageCallsPycompile;
use super::rules::MultiArchSamePackageHasArchSpecificOverrides;
use super::rules::MultilineArchitectureField;
use super::rules::NativeSourceFileWithoutUtf8Name;
use super::rules::NeedlessDependencyOnJre;
use super::rules::NewPackageShouldNotPackagePython2Module;
use super::rules::NewPackageUsesDateBasedVersionNumber;
use super::rules::NmuInChangelog;
use super::rules::NoDebianChanges;
use super::rules::NoShlibs;
use super::rules::NoTemplateName;
use super::rules::NodejsModuleInstalledInBadDirectory;
use super::rules::NonStandardApache2ModulePackageName;
use super::rules::NonStandardDirPerm;
use super::rules::NonStandardFilePerm;
use super::rules::NonStandardFilePermissionsForEtcInitDScript;
use super::rules::NonStandardSetuidExecutablePerm;
use super::rules::NotAllowedControlFile;
use super::rules::NotUsingPoDebconf;
use super::rules::ObsoleteCryptAlias;
use super::rules::ObsoleteDesEncryption;
use super::rules::ObsoleteFieldInDep5Copyright;
use super::rules::ObsoleteRelationForm;
use super::rules::ObsoleteRelationFormInSource;
use super::rules::ObsoleteRuntimeTestsRestriction;
use super::rules::OddPermissionsOnSharedLibrary;
use super::rules::OddPlaceForManualPage;
use super::rules::OmittedSystemdServiceForInitDScript;
use super::rules::OutdatedRelationInShlibs;
use super::rules::OverrideDhCleanDoesNotCallDhClean;
use super::rules::OverrideFileInWrongPackage;
use super::rules::PackageContainsCmakePrivateFile;
use super::rules::PackageContainsInfoDirFile;
use super::rules::PackageContainsMimeCacheFile;
use super::rules::PackageContainsMimeinfoCacheFile;
use super::rules::PackageContainsMultipleDpiFonts;
use super::rules::PackageContainsThumbnailsDir;
use super::rules::PackageContainsTimestampedGzip;
use super::rules::PackageContainsXvpicsDir;
use super::rules::PackageDependsOnHardcodedLibc;
use super::rules::PackageFileIsExecutable;
use super::rules::PackageInstallsAptKeyring;
use super::rules::PackageInstallsAptPreferences;
use super::rules::PackageInstallsAptSources;
use super::rules::PackageInstallsDeprecatedUpstartConfiguration;
use super::rules::PackageInstallsIntoEtcRcBoot;
use super::rules::PackageInstallsIntoEtcRcD;
use super::rules::PackageInstallsJavaBytecode;
use super::rules::PackageInstallsPacklist;
use super::rules::PackageInstallsPerllocalPod;
use super::rules::PackageInstallsPythonBytecode;
use super::rules::PackageInstallsPythonEgg;
use super::rules::PackageInstallsPythonPycacheDir;
use super::rules::PackageMixesMiscAndDpiFonts;
use super::rules::PackageModifiesLdSoSearchPath;
use super::rules::PackageNameDefinedInConfigH;
use super::rules::PackageNameDoesntMatchSonames;
use super::rules::PackageNotLowercase;
use super::rules::PackagePlaceholderInSymbolsFile;
use super::rules::PackageUsesDebhelperButLacksBuildDepends;
use super::rules::PackageUsesVendorSpecificPatchSeries;
use super::rules::PerlModuleUsesPerl4LibsWithoutDep;
use super::rules::PhpScriptWithUnusualInterpreter;
use super::rules::PkgJsAutopkgtestFileDoesNotExist;
use super::rules::PossibleMissingColonInCloses;
use super::rules::PossibleNewUpstreamReleaseWithoutNewVersion;
use super::rules::PostinstHasUselessCallToUpdateMenus;
use super::rules::PostrmCallsInstalldocs;
use super::rules::PostrmContainsAdditionalUpdatercDCalls;
use super::rules::PostrmDoesNotCallUpdatercDForInitDScript;
use super::rules::PostrmDoesNotPurgeDebconf;
use super::rules::PostrmHasUselessCallToUpdateMenus;
use super::rules::PreinstCallsInstalldocs;
use super::rules::PreinstCallsUpdatemenus;
use super::rules::PreinstCallsUpdatercD;
use super::rules::PrermCallsUpdatemenus;
use super::rules::PrermCallsUpdatercD;
use super::rules::PriorityExtraIsReplacedByPriorityOptional;
use super::rules::PrivacyBreachW3cValidHtml;
use super::rules::PuppetModuleButNotDhPuppet;
use super::rules::PypiHomepage;
use super::rules::Python3ScriptButNoPython3Dep;
use super::rules::PythonDebugInWrongLocation;
use super::rules::PythonModuleInWrongLocation;
use super::rules::QuiltBuildDepButNoSeriesFile;
use super::rules::QuiltPatchWithNonStandardOptions;
use super::rules::QuiltSeriesButNoBuildDep;
use super::rules::QuiltSeriesWithoutTrailingNewline;
use super::rules::RPackageNotArchAll;
use super::rules::RasterImageInScalableDirectory;
use super::rules::RcVersionGreaterThanExpectedVersion;
use super::rules::ReadmeDebianContainsDebmakeTemplate;
use super::rules::ReadmeSourceIsDhMakeTemplate;
use super::rules::RedundantBuildPrerequisites;
use super::rules::RedundantInstallationPrerequisite;
use super::rules::RelativeConffile;
use super::rules::RepeatedTriggerName;
use super::rules::RubyInterpreterIsDeprecated;
use super::rules::RubyScriptButNoRubyDep;
use super::rules::RubygemHomepage;
use super::rules::ScriptInEtcInitDNotRegisteredViaUpdateRcD;
use super::rules::ScriptNotExecutable;
use super::rules::ScriptUsesDeprecatedNodejsLocation;
use super::rules::ScriptUsesPerl4LibsWithoutDep;
use super::rules::ScriptWithLanguageExtension;
use super::rules::SectionIsDhMakeTemplate;
use super::rules::SelectWithoutChoices;
use super::rules::ServiceFileIsNotAFile;
use super::rules::SharedLibraryIsExecutable;
use super::rules::SharedLibraryIsMultiArchForeign;
use super::rules::SharedLibraryLacksStackSection;
use super::rules::SharedLibraryLacksVersion;
use super::rules::SharedLibraryNotShipped;
use super::rules::SharedLibrarySymbolsNotTracked;
use super::rules::ShipsUndeclaredSharedLibrary;
use super::rules::SourceContainsPrebuiltMsHelpFile;
use super::rules::SourceContainsPrebuiltSilverlightObject;
use super::rules::SourceContainsWafBinary;
use super::rules::SourceFieldMalformed;
use super::rules::SpecialFile;
use super::rules::SpecificAddressInSharedLibrary;
use super::rules::StrippedLibrary;
use super::rules::SurplusSharedLibrarySymbols;
use super::rules::SymlinkTargetInBuildTree;
use super::rules::SymlinkTargetInTmp;
use super::rules::SyntaxErrorInDebconfTemplate;
use super::rules::SyntaxErrorInDep5Copyright;
use super::rules::SystemdServiceFileRefersToUnusualWantedbyTarget;
use super::rules::SystemdServiceFileShutdownProblems;
use super::rules::SystemdServiceFileUsesDeprecatedSyslogFacility;
use super::rules::SystemdServiceInOddLocation;
use super::rules::TabInLicenseText;
use super::rules::TclshScriptButNoTclshDep;
use super::rules::TeamUploadHasIncorrectVersionNumber;
use super::rules::TemporaryDebhelperFile;
use super::rules::TestLeavesPythonVersionUntested;
use super::rules::TestsuiteDependencyHasUnparsableElements;
use super::rules::TestsuiteRestrictionsHasDeprecatedSkipNotInstallable;
use super::rules::TooManyArchitectures;
use super::rules::TrailingSlashForDpkgMaintscriptHelperSymlinkToDir;
use super::rules::TripletDirAndArchitectureMismatch;
use super::rules::TruetypeFontWrongFilename;
use super::rules::TypelibMissingGirDepends;
use super::rules::TypelibNotInMultiarchDirectory;
use super::rules::TypelibPackageNameDoesNotMatch;
use super::rules::TypelibSectionNotIntrospection;
use super::rules::UdebPostinstCallsLdconfig;
use super::rules::UdebUsesUnsupportedCompressionForDataTarball;
use super::rules::UdevRuleInEtc;
use super::rules::UdevRuleMissingSubsystem;
use super::rules::UdevRuleMissingUaccess;
use super::rules::UdevRuleUnreadable;
use super::rules::UnconditionalUseOfDpkgStatoverride;
use super::rules::UnexpectedConffile;
use super::rules::UnknownArchitecture;
use super::rules::UnknownConffileInstruction;
use super::rules::UnknownControlFile;
use super::rules::UnknownControlInterpreter;
use super::rules::UnknownEncodingInPoFile;
use super::rules::UnknownEssentialValue;
use super::rules::UnknownFileInDebianSource;
use super::rules::UnknownJavaClassVersion;
use super::rules::UnknownMetaFieldInSymbolsFile;
use super::rules::UnknownMultiArchValue;
use super::rules::UnknownParagraphInDep5Copyright;
use super::rules::UnknownPriority;
use super::rules::UnknownTemplateType;
use super::rules::UnknownTestsuite;
use super::rules::UnknownTrigger;
use super::rules::UnnecessaryTeamUpload;
use super::rules::UnparsableMenuItem;
use super::rules::UnsupportedSourceFormat;
use super::rules::UnusualControlInterpreter;
use super::rules::UnwantedPathTooSpecific;
use super::rules::UploaderNameMissing;
use super::rules::UpstartJobInEtcInitDNotRegisteredViaUpdateRcD;
use super::rules::UpstreamMetadataInNativeSource;
use super::rules::UpstreamMetadataIsNotAFile;
use super::rules::UselessAutoreconfBuildDepends;
use super::rules::UsesDeprecatedAdttmp;
use super::rules::UsesNoCompressionForControlTarball;
use super::rules::UsesNoCompressionForDataTarball;
use super::rules::UsrShareDocSymlinkPointsOutsideOfUsrShareDoc;
use super::rules::UsrShareDocSymlinkWithoutDependency;
use super::rules::VariesContentWithNocheck;
use super::rules::VersionRefersToDistribution;
use super::rules::VersionRefersToUnstableDistributionWithExpVersion;
use super::rules::VersionSubstvarForExternalPackage;
use super::rules::VimAddonWithinVimRuntimePath;
use super::rules::WaywardSymbolicLinkTargetInSource;
use super::rules::WeakLibraryDevDependency;
use super::rules::WebApplicationDependsOnApache2DataPackage;
use super::rules::WebApplicationWorksOnlyWithApache;
use super::rules::WhitespaceAfterContinuationCharacter;
use super::rules::WindowsDevelFileInPackage;
use super::rules::WishScriptButNoWishDep;
use super::rules::WoffFontInWrongDir;
use super::rules::WrongBugNumberInCloses;
use super::rules::WrongManualSection;
use super::rules::WrongNameForChangelogOfNativePackage;
use super::rules::WrongNameForDebianChangelogFile;
use super::rules::WrongNameForManualPage;
use super::rules::WrongNameForUpstreamChangelog;
use super::rules::WrongSectionForUdeb;
use super::rules::WrongTeam;
use super::rules::ZipParseError;
use super::rules::{
    AbsoluteSymlinkInTopLevelFolder, AliasedLocation, AncientLibtool, AncientPythonVersionField,
    Apache2ConfigurationFilesNeedConfSuffix, Apache2ReverseDependencyCallsWrapperScript,
    Apache2UnparsableDependency, ArchiveLibertyMismatch,
    AutotoolsPkgConfigMacroNotCrossCompilationSafe, BadHomepage, BadMenuFileName,
    BadProvidedPackageName, BadSoLinkWithinManualPage, BadWhatisEntry,
    BinaryArchRulesButPkgIsArchIndep, BogusMailHost, BogusMailHostInDebianChangelog,
    BogusPythonPrerequisite, BraceExpansionInDebhelperConfigFile, BrokenLinkToUndocumented,
    BuildConflictsWithBuildDependency, BuildDependsOn1Revision, BuildDependsOnBuildEssential,
    BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
    BuildDependsOnEssentialPackageWithoutUsingVersion, BuildDependsOnMetapackage,
    BuildDependsOnObsoleteBootstrap, BuildDependsOnObsoletePackage, BuildPathInManual,
    BuildPrerequisiteInInstallableSection, ChangelogFileNotCompressed, ChangelogNewsDebianMismatch,
    CircularInstallationPrerequisite, CommandInMenuFileAndDesktopFile,
    CommandWithPathInMaintainerScript, CompressedDocumentation, CompressedSymlinkWithWrongExt,
    ConflictsWithDependency, CopyrightContainsDhMakeTodoBoilerplate, CopyrightFileCompressed,
    CopyrightFileContainsFullGplLicense, CopyrightFileIsSymlink,
    CopyrightFileLacksPointerToPerlLicense, CopyrightNotUsingCommonLicenseForApache2,
    CopyrightNotUsingCommonLicenseForGpl, CopyrightNotUsingCommonLicenseForLgpl,
    CopyrightRefersToCompressedLicense, CopyrightRefersToDeprecatedBsdLicenseFile,
    CopyrightRefersToIncorrectDirectory, CopyrightRefersToNonexistentLicenseFile,
    CopyrightRefersToOldDirectory, CopyrightRefersToVersionlessLicenseFile, CountryInManual,
    CustomCompressionInDebianRules, DebhelperBaseCompatLevelIsMissing, DebhelperButNoMiscDepends,
    DebhelperToolsFromAutotoolsDevAreDeprecated, DebianAddsArchControlDir,
    DebianAddsArchInventoryFile, DebianAddsBtsControlDir, DebianAddsBzrControlDir,
    DebianAddsCvsConflictCopy, DebianAddsCvsControlDir, DebianAddsEditorBackupFile,
    DebianAddsGitControlDir, DebianAddsHgControlDir, DebianAddsHgTagsFile,
    DebianAddsPatchFailureFile, DebianAddsQuiltControlDir, DebianAddsSvkCommitFile,
    DebianAddsSvnCommitFile, DebianAddsSvnConflictFile, DebianAddsSvnControlDir,
    DebianChangelogFileIsASymlink, DebianChangelogFileMissingOrWrongName,
    DebianChangelogHasWrongDayOfWeek, DebianChangelogLineTooLong, DebianControlHasDbgsymPackage,
    DebianControlRepeatsFieldNameInValue, DebianNewsEntryHasStrangeDistribution,
    DebianRevisionIsZero, DebianRevisionNotWellFormed, DebianRulesCallsDebhelperInOddOrder,
    DebianRulesCallsNproc, DebianRulesCallsPwd, DebianRulesIgnoresMakeCleanError,
    DebianRulesMissingRequiredTarget,
    DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
    DebianTestsControlAutodep8IsObsolete, DebianWatchCouldVerifyDownload,
    DebianWatchFilePubkeyFileIsMissing, DebianWatchFileSpecifiesOldUpstreamVersion,
    DebianWatchFileSpecifiesWrongUpstreamVersion, DebianWatchFileUsesDeprecatedGithubredir,
    DebianWatchFileUsesDeprecatedSfRedirectorMethod, DebianWatchLacksSourceforgeRedirector,
    DebianWatchLineInvalid, DebianWatchNotManglingVersion, DebmagicDummyTrigger, DebugSuffixNotDbg,
    DependsExclusivelyOnMakedev, DependsOnEssentialPackageWithoutUsingVersion,
    DependsOnMetapackage, DependsOnObsoleteBootstrap, DependsOnObsoletePackage, DependsOnOldEmacs,
    DependsOnPackagingDev, DependsOnPythonMinimal, DescriptionContainsHomepage,
    DescriptionIsPkgName, DescriptionStartsWithPackageName, DescriptionSynopsisIsDuplicated,
    DescriptionSynopsisIsEmpty, DescriptionSynopsisStartsWithArticle, DescriptionTooShort,
    DesktopCommandNotInPackage, DesktopEntryContainsUnknownKey, DesktopEntryFileHasCrs,
    DesktopEntryInvalidCategory, DesktopEntryUsesReservedCategory, DirInUsrLocal,
    DirOrFileInEtcOpt, DirOrFileInHome, DirOrFileInMnt, DirOrFileInOpt, DirOrFileInRun,
    DirOrFileInSrv, DirOrFileInTmp, DirOrFileInVarLock, DirOrFileInVarRun, DirOrFileInVarWww,
    DmUploadAllowedIsObsolete, DocBaseAbstractFieldIsTemplate,
    DocBaseAbstractFieldSeparatorExtraWhitespace,
    DocBaseAbstractMightContainExtraLeadingWhitespace, DocBaseDocumentFieldEndsInWhitespace,
    DocBaseDocumentFieldNotInFirstLine, DocBaseFileLacksRequiredField, DocBaseFileNoFormat,
    DocBaseFileNoFormatSection, DocBaseFileNoIndex, DocBaseFileReferencesWrongPath,
    DocBaseFileSeparatorExtraWhitespace, DocBaseFileSyntaxError, DocBaseFileUnknownField,
    DocBaseFileUnknownFormat, DocBaseIndexReferencesMultipleFiles, DocBaseInvalidDocumentField,
    DocBaseUnknownSection, DocBaseUsesApplicationsSection, DuplicateEntryInSymbolsControlFile,
    DuplicateFieldInDocBase, DuplicateFormatInDocBase, DuplicateInShlibs,
    EmbeddedFeedparserLibrary, EmptyBinaryPackage, EmptyManualPage,
    EmptyShortLicenseInDep5Copyright, EpochChangeWithoutComment,
    EpochChangedButUpstreamVersionDidNotGoBackwards, EssentialNoNotNeeded,
    ExcessivePriorityForLibraryPackage, ExtendedDescriptionContainsEmptyParagraph,
    ExtendedDescriptionIsEmpty, ExtendedDescriptionLineTooLong, FaultyDebianQaGroupAddress,
    FaultyDebianQaGroupPhrase, FieldTooLong, FileDirectlyInUsrShare, FileDirectlyInUsrShareDoc,
    FileInEtcNotMarkedAsConffile, FileInUnusualDir, FileInUsrLibSgml, FileInUsrLibSitePython,
    FileInUsrLocal, FileInUsrMarkedAsConffile, FileMissingInMd5sums,
    FileNameContainsWildcardCharacter, FileNameEndsInWhitespace, FileNameInPathIsNotAscii,
    ForbiddenConfigInterpreter, Format30ButDebianChangesPatch, FsstndDirInUsr, FsstndDirInVar,
    GirSectionNotLibdevel, GroffMessage, HeaderHasOverlyGenericName,
    HomepageForCpanPackageContainsVersion, HomepageRefersToObsoleteDebianInfrastructure,
    HyphenFile, IconSizeAndDirectoryNameMismatch, IllegalVariable,
    IncompleteCreativeCommonsLicense, InfoDocumentMissingDirEntry, InfoDocumentMissingDirSection,
    InitDScriptDependsOnAllVirtualFacility, InitDScriptDoesNotImplementRequiredOption,
    InitDScriptDoesNotSourceInitFunctions, InitDScriptHasBadLsbLine, InitDScriptHasBadStopRunlevel,
    InitDScriptHasConflictingStartStop, InitDScriptMissingLsbSection,
    InitDScriptPossibleMissingStop, InitDScriptShouldAlwaysStartService,
    InitDScriptStartsInStopRunlevel, InitDScriptStopsInSRunlevel, InitialUploadClosesNoBugs,
    InstallInfoUsedInMaintainerScript, InvalidArchStringInSourceRelation,
    InvalidDateInDebianChangelog, InvalidPoFile, InvalidShortNameInDep5Copyright,
    InvalidTemplateIdInSymbolsFile, LatestChangelogEntryWithoutNewDate,
    LatestDebianChangelogEntryReusesExistingVersion, LatestDebianChangelogEntryWithoutNewDate,
    LengthySymlink, LicenseProblemUndefinedLicense, LoadsObsoleteConfmodule,
    MacosDsStoreFileInPackage, MacosResourceForkFileInPackage, MailAddressLoopsOrBounces,
    MailTransportAgentDependencyDoesNotSpecifyDefaultMta, MaintainerScriptCallsChownImproperly,
    MaintainerScriptCallsGconftool, MaintainerScriptCallsInitScriptDirectly,
    MaintainerScriptCallsInstallSgmlcatalog, MaintainerScriptCallsStartStopDaemon,
    MaintainerScriptChangesLdSoConf, MaintainerScriptChangesNetbase, MaintainerScriptEmpty,
    MaintainerScriptHasInvalidUpdateInetdOptions, MaintainerScriptHasUnexpandedDebhelperToken,
    MaintainerScriptHidesInitFailure, MaintainerScriptIgnoresErrors,
    MaintainerScriptLacksDebhelperToken, MaintainerScriptModifiesInetdConf,
    MaintainerScriptNeedsDependsOnAdduser, MaintainerScriptNeedsDependsOnGconf2,
    MaintainerScriptNeedsDependsOnUcf, MaintainerScriptNeedsDependsOnUpdateInetd,
    MaintainerScriptNeedsDependsOnXfontsUtils, MaintainerScriptNeedsDependsOnXmlCore,
    MaintainerScriptRemovesDeviceFiles, MaintainerScriptShouldNotParseEtcPasswdOrGroup,
    MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
    MaintainerScriptShouldNotUsePiupartsVariable, MaintainerScriptUpdatesFontconfigCacheImproperly,
    MaintscriptCallsLdconfig, MaintscriptIncludesMaintScriptParameters,
    MakingAssumptionsAboutInterfacesInTemplates, MalformedDmUploadAllowed,
    MalformedMd5sumsControlFile, MalformedPromptInTemplates, MalformedPythonVersion,
    MalformedQuestionInTemplates, MalformedTitleInTemplates, ManualPageFromTemplate,
    ManualPageWithGenericName, Md5sumMismatch, Md5sumsListsNonexistentFile,
    MenuCommandNotInPackage, MenuFileInUsrLib, MenuIconMissing, MenuIconUsesRelativePath,
    MenuItemCreatesNewSection, MenuItemMissingRequiredTag, MenuItemNeedsTagHasUnknownValue,
    MenuItemUsesAppsSection, MenuItemUsesWindowmanagersSection, MenuMethodLacksInclude,
    MismatchTranslatedChoices, MisnamedPoFile, MissingCallToDpkgMaintscriptHelper, MissingConffile,
    MissingDebconfDependencyForPreinst, MissingDebianWatchFileStandard, MissingDependencyOnPerlapi,
    MissingFieldInDep5Copyright, MissingLicenseParagraphInDep5Copyright,
    MissingLicenseTextInDep5Copyright, MissingSeparatorBetweenItems,
    MissingSystemdServiceForInitDRcSScript, MissingSystemdServiceForInitDScript,
    MissingSystemdTimerForCronScript, MultipleDebianWatchFileStandards,
    NamedCopyrightForSingleInstallable, NationalEncoding, NeedlessSuggestRecommendLibservletJava,
    NeedlesslyDependsOnAwk, NewEssentialPackage, NoChangelog, NoCopyrightFile, NoDebconfConfig,
    NoDebconfTemplates, NoDebianCopyrightInSource, NoDep5Copyright, NoEnglishManualPage,
    NoHumanMaintainers, NoManualPage, NoNmuInChangelog, NoOpTestsuite, NoQaInChangelog,
    NoTemplateDescription, NoTemplateType, NoVersionedDebhelperPrerequisite,
    NodePackageInstallInNodejsRootdir, NodejsLockFile, NodejsMissingVersionOverride,
    NodejsModuleInstalledInUsrLib, NodejsModuleNotDeclared, NonDebugFileInDebugPackage,
    NonEtcFileMarkedAsConffile, NonMultiArchLibDir, NonStandardApache2ConfigurationName,
    NonStandardDirInUsr, NonStandardDirInVar, NonStandardToplevelDir,
    NonVirtualFacilityInInitdScript, NonWmInWindowmanagerMenuSection, NotBinnmuableAllDependsAny,
    NotBinnmuableAnyDependsAll, NotBinnmuableAnyDependsAny, ObsoleteCommandInModprobeDFile,
    ObsoleteDebianWatchFileStandard, ObsoleteUrlInPackaging, OddHistoricalDebianChangelogVersion,
    OldFsfAddressInCopyrightFile, OverrideDhFixpermsDoesNotCallDhFixperms,
    OverrideFileInWrongLocation, PackageContainsBtsControlDir, PackageContainsCompiledFontFile,
    PackageContainsCompiledGlibSchema, PackageContainsEslintConfigFile,
    PackageContainsFileInEtcSkel, PackageContainsFileInUsrShareHal,
    PackageContainsIconCacheInGenericDir, PackageContainsLindaOverride,
    PackageContainsNpmIgnoreFile, PackageContainsPythonCoverageFile,
    PackageContainsPythonDoctreeFile, PackageContainsPythonDotDirectory,
    PackageContainsPythonHeaderInIncorrectDirectory, PackageContainsPythonHypothesisExample,
    PackageContainsPythonTestsInGlobalNamespace, PackageContainsReadmeForOtherPlatformOrDistro,
    PackageContainsSassCacheDirectory, PackageContainsUnsafeSymlink, PackageContainsVcsControlDir,
    PackageContainsVcsControlFile, PackageDependsOnMultipleLibpngVersions,
    PackageDependsOnMultipleLibstdcVersions, PackageDependsOnMultipleTclVersions,
    PackageDependsOnMultipleTkVersions, PackageInstallsAppimage,
    PackageInstallsDeprecatedPython2Path, PackageInstallsFileToUsrX11r6, PackageInstallsIeeeData,
    PackageInstallsIntoEtcGconfSchemas, PackageInstallsNonbinaryPerlInUsrLibPerl5,
    PackageRelationWithPerlModules, PackageRelationWithSelf, PackageSectionGamesButContainsNoGame,
    PackageSectionGamesButHasUsrBin, PackageUsesDeprecatedDebhelperCompatVersion,
    PackageUsesDhExecButLacksBuildDepends, PackageUsesLocalDiversion, PkgJsToolsTestIsEmpty,
    PkgJsToolsTestIsMissing, PkgNotInPackageTest, PodConversionMessage,
    PoorCompressionInManualPage, PossibleDebconfNoteAbuse,
    PossiblyInsecureHandlingOfTmpFilesInMaintainerScript, PostinstDoesNotCallUpdatemenus,
    PostinstDoesNotLoadConfmodule, PostinstHasUselessCallToInstallDocs, PostinstUsesDbInput,
    PostrmDoesNotCallUpdatemenus, PostrmRemovesAlternative, PrermHasUselessCallToInstallDocs,
    PythonModuleHasOverlyGenericName, PythonPackageMissingDependsOnPython,
    PythonVersionCurrentIsDeprecated, QaUploadHasIncorrectVersionNumber,
    QuotedPlaceholderInMailcapEntry, ReadInMaintainerScript,
    ReadmeDebianContainsInvalidEmailAddress, ReadmeDebianMentionsUsrDoc, RecommendedField,
    RecursivePrivilegeChange, RedundantBugsField, RedundantOriginField, RelativeSymlink,
    RequiredField, RunPartsCronFilenameContainsIllegalChars,
    RuntimeTestFileUsesInstalledPythonVersions, ScriptUsesBinEnv, SelectWithBooleanChoices,
    SourceContainsCmakeCacheFile, SourceContainsDebianSubstvars,
    SourceContainsPrebuiltWindowsBinary, SourceNmuHasIncorrectVersionNumber,
    SourcePackageEncodesPythonVersion, SourceShipsExcludedFile, SpaceInStdShortnameInDep5Copyright,
    StarFile, StrayTranslatedDebconfTemplates, SuWrapperNotSuToRoot, SuWrapperWithoutC,
    SubdirInUsrBin, SuperfluousClutterInHomepage, SuperfluousFilePattern, SvkCommitFileInPackage,
    SvnCommitFileInPackage, SymbolsFileContainsCurrentVersionWithDebianRevision,
    SymbolsFileContainsDebianRevision, SymbolsForUndeclaredSharedLibrary,
    SymlinkContainsSpuriousSegments, SymlinkEndsWithSlash, SymlinkHasDoubleSlash,
    SymlinkHasTooManyUpSegments, SymlinkIsSelfRecursive, SynopsisTooLong,
    SyntaxErrorInDebianChangelog, SyntaxErrorInDebianNewsFile, SyntaxErrorInSymbolsFile,
    SystemdAlternatives, SystemdServiceAliasWithoutExtension,
    SystemdServiceFileRefersToObsoleteBindto, SystemdServiceFileRefersToObsoleteTarget,
    SystemdServiceFileUsesNobodyOrNogroup, SystemdServiceFileWrapsInitScript,
    TemplateUsesUnsplitChoices, ThirdPartyPackageInPythonDir,
    TooLongExtendedDescriptionInTemplates, TooLongShortDescriptionInTemplates,
    TranslatedDefaultField, TypoInDebhelperOverrideTarget, UdevadmCalledWithoutGuard,
    UncompressedManualPage, UndocumentedManualPage, UnknownDebconfPriority,
    UnknownDebianWatchFileStandard, UnknownField, UnknownFieldInTemplates,
    UnknownFileInPythonModuleDirectory, UnknownSection, UnnecessaryTestsuiteAutopkgtestField,
    UnquotedStringInMenuItem, UnstrippedBinaryOrObject, UntranslatableDebconfTemplates,
    UploadersInOrphan, UseOfCompatSymlink, UselessWhatisEntry, UsesDebhelperCompatFile,
    UsesDpkgDatabaseDirectly, UsesImplicitAwaitTrigger, UsingFirstPersonInTemplates,
    UsingImperativeFormInTemplates, UsingQuestionInExtendedDescriptionInTemplates,
    VcsFieldUsesNotRecommendedUriFormat, VcsObsoleteInDebianInfrastructure,
    VirtualPackageDependsWithoutRealPackageDepends, WindowsThumbnailDatabaseInPackage,
    WrongCompressionInManualPage, WrongNameForDebianNewsFile, WrongPackageMailAlias,
};
use super::rules::{
    EmbeddedJavascriptLibrary, EmbeddedPhpLibrary, NonFreeFlash, PrivacyBreachDonation,
    PrivacyBreachFacebook, PrivacyBreachGeneric, PrivacyBreachGoogleAdsense,
    PrivacyBreachGoogleCse, PrivacyBreachGooglePlus, PrivacyBreachLogo, PrivacyBreachPiwik,
    PrivacyBreachStatisticsWebsite, PrivacyBreachTwitter, PrivacyBreachUsesEmbeddedFile,
};

use super::rules::NonStandardExecutablePerm;

use super::rules::ConfigFileReserved;
use super::rules::ExecutableInUsrShareDoc;
use super::rules::IncorrectNamingOfPkcs11Module;
use super::rules::NestedExamplesDirectory;
use super::rules::PackageContainsHardlink;
use super::rules::PackageContainsMimeFileOutsidePackageDir;
use super::rules::PackageInstallsIntoObsoleteDir;

use super::rules::MissingDepForInterpreter;
use super::rules::ShippedFileWithoutUtf8Name;
use super::rules::UnusualInterpreter;

use super::rules::NonWmModuleInWmModulesMenuSection;

use super::rules::SuToRootWithUsrSbin;

use super::rules::BadTestInMenuItem;

use super::rules::DuplicateTagInMenu;

use super::rules::DesktopContainsDeprecatedKey;

use super::rules::DesktopEntryContainsDeprecatedKey;

use super::rules::DuplicateKeyInDesktop;

use super::rules::DesktopEntryUnknownType;

use super::rules::DesktopEntryMissingRequiredKey;

use super::rules::DesktopEntryLacksMainCategory;

use super::rules::DesktopMimeButNoExecCode;

use super::rules::FileInDiscouragedX11FontDirectory;

use super::rules::FileInUnknownX11FontDirectory;

use super::rules::DesktopFileInWrongDir;

use super::rules::EmptyDebianTestsControl;

use super::rules::ConflictingTestFields;

use super::rules::BackgroundedTestCommand;

use super::rules::IllegalRuntimeTestName;

use super::rules::MissingRuntimeTestFile;

use super::rules::RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends;

use super::rules::DeclarePythonVersionsForTest;

use super::rules::DropPythonVersionDeclaration;

use super::rules::TestsuiteDependsOnObsoletePackage;

use super::rules::BooleanTemplateHasBogusDefault;

use super::rules::EmptyTranslatedChoices;

use super::rules::DebianNewsFileNotCompressed;
use super::rules::DuplicateLongDescriptionInTemplate;

use super::rules::ChangelogEmptyEntry;

use super::rules::ChangelogReferencesTempSecurityIdentifier;

use super::rules::ImprobableBugNumberInCloses;

use super::rules::DebianChangelogFileContainsObsoleteUserEmacsSettings;

use super::rules::DebianNewsEntryHasUnknownVersion;

use super::rules::HtmlChangelogWithoutTextVersion;

use super::rules::EssentialInSourcePackage;

use super::rules::PatchFilePresentButNotMentionedInSeries;
use super::rules::PatchSystemButNoSourceReadme;
use super::rules::RootInContact;

use super::rules::ExplicitDefaultInPackageType;

use super::rules::DescriptionContainsTabs;

use super::rules::DescriptionContainsInvalidControlStatement;

use super::rules::DescriptionContainsDhMakePerlTemplate;

use super::rules::DescriptionIsDebmakeTemplate;

use super::rules::DescriptionStartsWithLeadingSpaces;

use super::rules::BackupFileInPackage;
use super::rules::DirOrFileInBuildTree;
use super::rules::DiversionForUnknownFile;
use super::rules::ExecutableNotElfOrScript;
use super::rules::GameOutsideSection;
use super::rules::GlobalDataInGamesDirectory;
use super::rules::InvalidStandardsVersion;
use super::rules::MaintainerAlsoInUploaders;
use super::rules::MissingBuildDependencyForDhAddon;
use super::rules::MissingBuildDependencyForDhCommand;
use super::rules::MultiarchForeignPkgconfig;
use super::rules::NewerStandardsVersion;
use super::rules::NfsTemporaryFileInPackage;
use super::rules::OpentypeFontProhibitsInstallableEmbedding;
use super::rules::OpentypeFontWrongFilename;
use super::rules::OrphanedDiversion;
use super::rules::PkgConfigBadDirective;
use super::rules::RelativeLibrarySearchPath;
use super::rules::RulesRequireBuildPrerequisite;
use super::rules::ScriptWithoutInterpreter;
use super::rules::StaticallyLinkedBinary;
use super::rules::TimewarpStandardsVersion;
use super::rules::TruetypeFontProhibitsInstallableEmbedding;
use super::rules::WrongPathForInterpreter;

use super::rules::AbsoluteSymbolicLinkTargetInSource;
use super::rules::AdduserWithHomeVarRun;
use super::rules::ArchDependentFileInUsrShare;
use super::rules::ArchDependentFileNotInArchSpecificDirectory;
use super::rules::AspellPackageNotArchAll;
use super::rules::BadMenuItem;
use super::rules::BadPermissionsForEtcEmacsScript;
use super::rules::BugsFieldDoesNotReferToDebianInfrastructure;
use super::rules::ConffileHasBadFileType;
use super::rules::CustomCompressionInDebianSourceOptions;
use super::rules::DfsgVersionInNativePackage;
use super::rules::DfsgVersionMisspelled;
use super::rules::DirectoryInModprobeD;
use super::rules::DocumentationPackageNotArchitectureIndependent;
use super::rules::DuplicateChangelogFiles;
use super::rules::DuplicateConffile;
use super::rules::DuplicateContact;
use super::rules::DuplicateP3distOverride;
use super::rules::DuplicatePackagingFile;
use super::rules::EmacsenCommonWithoutDhElpa;
use super::rules::FileInEtcRcDMarkedAsConffile;
use super::rules::FileInRootAndUsr;
use super::rules::GitPatchesNotExported;
use super::rules::IncorrectLibdirInLaFile;
use super::rules::IncorrectPackagingFilename;
use super::rules::InfoDocumentHasWrongExtension;
use super::rules::InfoDocumentMissingImageFile;
use super::rules::InfoDocumentNotCompressed;
use super::rules::InfoDocumentNotCompressedWithGzip;
use super::rules::InfoDocumentNotCompressedWithMaxCompression;
use super::rules::InitDScriptHasBadStartRunlevel;
use super::rules::InitDScriptHasDuplicateLsbKeyword;
use super::rules::InitDScriptHasDuplicateLsbSection;
use super::rules::InitDScriptHasUnknownLsbKeyword;
use super::rules::InitDScriptHasUnterminatedLsbSection;
use super::rules::InitDScriptMissingDependencyOnLocalFs;
use super::rules::InitDScriptMissingLsbKeyword;
use super::rules::InitDScriptMissingStart;
use super::rules::InitDScriptNotIncludedInPackage;
use super::rules::InitDScriptNotMarkedAsConffile;
use super::rules::InitDScriptProvidesVirtualFacility;
use super::rules::InitDScriptSourcingWithoutTest;
use super::rules::InvalidDep3FormatPatchBugFieldDefaultValue;
use super::rules::InvalidDep3FormatPatchBugNotContainUrl;
use super::rules::InvalidDep3FormatPatchMaybeBugDebian;
use super::rules::InvalidDep3FormatPatchOriginFieldDefaultValue;
use super::rules::InvalidValueInBuiltUsingField;
use super::rules::InvalidValueInStaticBuiltUsingField;
use super::rules::KillallIsDangerous;
use super::rules::LibraryInRootAndUsr;
use super::rules::MaintainerScriptCallsSystemctl;
use super::rules::MissingDebianSourceFormat;
use super::rules::MknodInMaintainerScript;
use super::rules::MultiarchForeignCmakeFile;
use super::rules::MultiarchForeignSharedLibrary;
use super::rules::MultiarchForeignStaticLibrary;
use super::rules::NoNewlineAtEnd;
use super::rules::NonConfFileInModprobeD;
use super::rules::NonEmptyDependencyLibsInLaFile;
use super::rules::ObsoleteCommentsStyleInPhpIni;
use super::rules::OcamlDanglingCmxa;
use super::rules::OcamlDanglingCmxs;
use super::rules::OddStaticLibraryName;
use super::rules::OldDevhelpStandard;
use super::rules::OldStyleConfigScriptMultiarchPath;
use super::rules::OldStyleConfigScriptMultiarchPathArchAll;
use super::rules::OrphanedPackageMaintainedInPrivateSpace;
use super::rules::OrphanedPackageNotMaintainedInDebianInfrastructure;
use super::rules::PerlModuleInCoreDirectory;
use super::rules::PhpScriptButNoPhpCliDep;
use super::rules::PkgConfigMultiArchWrongDir;
use super::rules::PkgConfigUnavailableForCrossCompilation;
use super::rules::RDataWithoutReadmeSource;
use super::rules::RemoveOfUnknownDiversion;
use super::rules::ScriptUsesUnversionedPythonInShebang;
use super::rules::SharedobjectInLibraryDirectoryMissingSoname;
use super::rules::SystemdDiversion;
use super::rules::VcsFieldBitrotted;
use super::rules::VcsFieldMismatch;
use super::rules::ZeroByteExecutableInPath;
use super::rules::ZeroByteFileInDocDirectory;

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
        &MalformedDmUploadAllowed,
        &SymlinkContainsSpuriousSegments,
        &SymlinkIsSelfRecursive,
        &LengthySymlink,
        &WrongPackageMailAlias,
        &DebianChangelogFileIsASymlink,
        &PackageContainsVcsControlDir,
        &DirOrFileInSrv,
        &DirOrFileInMnt,
        &ExtendedDescriptionIsEmpty,
        &DirOrFileInHome,
        &DirOrFileInTmp,
        &DirOrFileInOpt,
        &DirOrFileInVarWww,
        &DirOrFileInVarRun,
        &DirOrFileInVarLock,
        &DirOrFileInRun,
        &DirOrFileInEtcOpt,
        &DirInUsrLocal,
        &FileInUsrLocal,
        &FileDirectlyInUsrShare,
        &SubdirInUsrBin,
        &FsstndDirInUsr,
        &NonStandardDirInUsr,
        &FsstndDirInVar,
        &NonStandardDirInVar,
        &AliasedLocation,
        &UseOfCompatSymlink,
        &DescriptionSynopsisIsEmpty,
        &DescriptionStartsWithPackageName,
        &DescriptionSynopsisStartsWithArticle,
        &DescriptionTooShort,
        &DescriptionIsPkgName,
        &SynopsisTooLong,
        &ExtendedDescriptionLineTooLong,
        &SvnCommitFileInPackage,
        &AbsoluteSymlinkInTopLevelFolder,
        &FileNameEndsInWhitespace,
        &SourceContainsCmakeCacheFile,
        &FileNameContainsWildcardCharacter,
        &RelativeSymlink,
        &SymlinkEndsWithSlash,
        &SymlinkHasDoubleSlash,
        &SymlinkHasTooManyUpSegments,
        &PackageContainsPythonDotDirectory,
        &PackageContainsPythonHypothesisExample,
        &PackageContainsSassCacheDirectory,
        &CompressedSymlinkWithWrongExt,
        &HyphenFile,
        &FileNameInPathIsNotAscii,
        &PackageContainsCompiledGlibSchema,
        &StarFile,
        &PackageContainsFileInEtcSkel,
        &PackageContainsFileInUsrShareHal,
        &PackageContainsIconCacheInGenericDir,
        &PackageContainsPythonCoverageFile,
        &PackageContainsPythonDoctreeFile,
        &PackageContainsPythonHeaderInIncorrectDirectory,
        &PackageContainsPythonTestsInGlobalNamespace,
        &PackageContainsEslintConfigFile,
        &PackageContainsNpmIgnoreFile,
        &MissingDebianWatchFileStandard,
        &ObsoleteDebianWatchFileStandard,
        &UnknownDebianWatchFileStandard,
        &MultipleDebianWatchFileStandards,
        &AncientPythonVersionField,
        &DebianWatchLineInvalid,
        &DebianWatchLacksSourceforgeRedirector,
        &DebianWatchFileUsesDeprecatedGithubredir,
        &DebianWatchFileUsesDeprecatedSfRedirectorMethod,
        &DebianWatchNotManglingVersion,
        &DebianWatchFileSpecifiesWrongUpstreamVersion,
        &DebianWatchFileSpecifiesOldUpstreamVersion,
        &DebianWatchFilePubkeyFileIsMissing,
        &DebianWatchCouldVerifyDownload,
        &MalformedPythonVersion,
        &PythonVersionCurrentIsDeprecated,
        &SourcePackageEncodesPythonVersion,
        &BogusMailHost,
        &DependsOnObsoletePackage,
        &DependsOnOldEmacs,
        &NotBinnmuableAllDependsAny,
        &NotBinnmuableAnyDependsAny,
        &NotBinnmuableAnyDependsAll,
        &OldFsfAddressInCopyrightFile,
        &CopyrightRefersToOldDirectory,
        &CopyrightRefersToDeprecatedBsdLicenseFile,
        &CopyrightRefersToCompressedLicense,
        &CopyrightRefersToIncorrectDirectory,
        &CopyrightRefersToVersionlessLicenseFile,
        &CopyrightRefersToNonexistentLicenseFile,
        &CopyrightFileCompressed,
        &CopyrightFileIsSymlink,
        &SpaceInStdShortnameInDep5Copyright,
        &FileInEtcNotMarkedAsConffile,
        &NoDebconfConfig,
        &UnknownField,
        &MissingSystemdServiceForInitDScript,
        &InitDScriptDoesNotSourceInitFunctions,
        &NonMultiArchLibDir,
        &PackageContainsVcsControlFile,
        &PackageInstallsDeprecatedPython2Path,
        &FileInUsrLibSitePython,
        &ThirdPartyPackageInPythonDir,
        &MenuFileInUsrLib,
        &BadMenuFileName,
        &SvkCommitFileInPackage,
        &VcsFieldUsesNotRecommendedUriFormat,
        &DebhelperBaseCompatLevelIsMissing,
        &TypoInDebhelperOverrideTarget,
        &MaintainerScriptLacksDebhelperToken,
        &DebianAddsArchControlDir,
        &DebianAddsArchInventoryFile,
        &DebianAddsBtsControlDir,
        &DebianAddsBzrControlDir,
        &DebianAddsCvsConflictCopy,
        &DebianAddsCvsControlDir,
        &DebianAddsEditorBackupFile,
        &DebianAddsGitControlDir,
        &DebianAddsHgControlDir,
        &DebianAddsHgTagsFile,
        &DebianAddsPatchFailureFile,
        &DebianAddsSvkCommitFile,
        &DebianAddsSvnCommitFile,
        &DebianAddsSvnConflictFile,
        &DebianAddsSvnControlDir,
        &DebianAddsQuiltControlDir,
        &DebianRulesCallsPwd,
        &DebianRulesCallsDebhelperInOddOrder,
        &BinaryArchRulesButPkgIsArchIndep,
        &RedundantBugsField,
        &BadHomepage,
        &DescriptionContainsHomepage,
        &DescriptionSynopsisIsDuplicated,
        &FieldTooLong,
        &MissingSeparatorBetweenItems,
        &MaintainerScriptIgnoresErrors,
        &ReadInMaintainerScript,
        &RecursivePrivilegeChange,
        &CommandWithPathInMaintainerScript,
        &MaintainerScriptRemovesDeviceFiles,
        &PackageContainsBtsControlDir,
        &UsesDpkgDatabaseDirectly,
        &ObsoleteUrlInPackaging,
        &MaintainerScriptCallsInstallSgmlcatalog,
        &MaintainerScriptChangesNetbase,
        &MaintscriptCallsLdconfig,
        &MaintainerScriptCallsChownImproperly,
        &MaintainerScriptModifiesInetdConf,
        &MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
        &InstallInfoUsedInMaintainerScript,
        &MaintainerScriptHidesInitFailure,
        &MaintainerScriptNeedsDependsOnGconf2,
        &MaintainerScriptNeedsDependsOnUcf,
        &MaintainerScriptNeedsDependsOnXmlCore,
        &MaintainerScriptNeedsDependsOnXfontsUtils,
        &MaintainerScriptCallsInitScriptDirectly,
        &MaintainerScriptShouldNotParseEtcPasswdOrGroup,
        &InvalidArchStringInSourceRelation,
        &DependsOnMetapackage,
        &DependsOnPackagingDev,
        &DependsOnPythonMinimal,
        &NeedlesslyDependsOnAwk,
        &PackageDependsOnMultipleLibstdcVersions,
        &BuildDependsOnEssentialPackageWithoutUsingVersion,
        &PackageContainsReadmeForOtherPlatformOrDistro,
        &CompressedDocumentation,
        &FileDirectlyInUsrShareDoc,
        &MacosDsStoreFileInPackage,
        &MacosResourceForkFileInPackage,
        &PackageInstallsAppimage,
        &DebugSuffixNotDbg,
        &PackageContainsLindaOverride,
        &DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
        &DebhelperToolsFromAutotoolsDevAreDeprecated,
        &DebianRulesCallsNproc,
        &OverrideDhFixpermsDoesNotCallDhFixperms,
        &BraceExpansionInDebhelperConfigFile,
        &PackageUsesDhExecButLacksBuildDepends,
        &DebianControlHasDbgsymPackage,
        &BuildPrerequisiteInInstallableSection,
        &InitDScriptDoesNotImplementRequiredOption,
        &InitDScriptStopsInSRunlevel,
        &InitDScriptMissingLsbSection,
        &SystemdServiceFileRefersToObsoleteTarget,
        &MissingSystemdServiceForInitDRcSScript,
        &SystemdServiceFileWrapsInitScript,
        &MaintainerScriptNeedsDependsOnAdduser,
        &MaintainerScriptNeedsDependsOnUpdateInetd,
        &MaintainerScriptCallsGconftool,
        &MaintainerScriptCallsStartStopDaemon,
        &NoManualPage,
        &UncompressedManualPage,
        &WrongCompressionInManualPage,
        &CountryInManual,
        &BuildPathInManual,
        &BrokenLinkToUndocumented,
        &VirtualPackageDependsWithoutRealPackageDepends,
        &BuildDependsOnBuildEssential,
        &BuildDependsOn1Revision,
        &DependsExclusivelyOnMakedev,
        &BuildDependsOnObsoletePackage,
        &BuildDependsOnMetapackage,
        &UnknownFieldInTemplates,
        &PossibleDebconfNoteAbuse,
        &MalformedQuestionInTemplates,
        &UnknownDebconfPriority,
        &UsingImperativeFormInTemplates,
        &LoadsObsoleteConfmodule,
        &MakingAssumptionsAboutInterfacesInTemplates,
        &MalformedPromptInTemplates,
        &MalformedTitleInTemplates,
        &MismatchTranslatedChoices,
        &MissingDebconfDependencyForPreinst,
        &NoDebconfTemplates,
        &MenuCommandNotInPackage,
        &CommandInMenuFileAndDesktopFile,
        &MenuItemCreatesNewSection,
        &DesktopCommandNotInPackage,
        &DesktopEntryContainsUnknownKey,
        &MenuItemUsesAppsSection,
        &MenuItemUsesWindowmanagersSection,
        &NonWmInWindowmanagerMenuSection,
        &DesktopEntryFileHasCrs,
        &DesktopEntryInvalidCategory,
        &DesktopEntryUsesReservedCategory,
        &PkgNotInPackageTest,
        &ConflictsWithDependency,
        &MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
        &BuildConflictsWithBuildDependency,
        &PackageDependsOnMultipleLibpngVersions,
        &PackageDependsOnMultipleTclVersions,
        &PackageDependsOnMultipleTkVersions,
        &PackageRelationWithPerlModules,
        &BadProvidedPackageName,
        &BuildDependsOnObsoleteBootstrap,
        &DependsOnObsoleteBootstrap,
        &GroffMessage,
        &EmptyManualPage,
        &UndocumentedManualPage,
        &BadSoLinkWithinManualPage,
        &BadWhatisEntry,
        &ManualPageFromTemplate,
        &ManualPageWithGenericName,
        &NoEnglishManualPage,
        &PodConversionMessage,
        &UselessWhatisEntry,
        &CopyrightFileLacksPointerToPerlLicense,
        &NoDebianCopyrightInSource,
        &CopyrightContainsDhMakeTodoBoilerplate,
        &CopyrightNotUsingCommonLicenseForGpl,
        &CopyrightFileContainsFullGplLicense,
        &CopyrightNotUsingCommonLicenseForApache2,
        &CopyrightNotUsingCommonLicenseForLgpl,
        &NamedCopyrightForSingleInstallable,
        &SystemdServiceAliasWithoutExtension,
        &SystemdServiceFileRefersToObsoleteBindto,
        &SystemdServiceFileUsesNobodyOrNogroup,
        &InitDScriptHasConflictingStartStop,
        &InitDScriptHasBadStopRunlevel,
        &InitDScriptShouldAlwaysStartService,
        &NonVirtualFacilityInInitdScript,
        &InitDScriptDependsOnAllVirtualFacility,
        &InitDScriptPossibleMissingStop,
        &InitDScriptStartsInStopRunlevel,
        &PostrmRemovesAlternative,
        &ScriptUsesBinEnv,
        &MaintainerScriptShouldNotUsePiupartsVariable,
        &SystemdAlternatives,
        &MaintainerScriptHasInvalidUpdateInetdOptions,
        &UdevadmCalledWithoutGuard,
        &MaintainerScriptChangesLdSoConf,
        &MaintainerScriptUpdatesFontconfigCacheImproperly,
        &EpochChangedButUpstreamVersionDidNotGoBackwards,
        &EpochChangeWithoutComment,
        &LatestDebianChangelogEntryReusesExistingVersion,
        &OddHistoricalDebianChangelogVersion,
        &DebianChangelogHasWrongDayOfWeek,
        &DebianChangelogLineTooLong,
        &InitialUploadClosesNoBugs,
        &NoNmuInChangelog,
        &SourceNmuHasIncorrectVersionNumber,
        &NoQaInChangelog,
        &NoTemplateDescription,
        &NoTemplateType,
        &SelectWithBooleanChoices,
        &TemplateUsesUnsplitChoices,
        &TooLongExtendedDescriptionInTemplates,
        &TooLongShortDescriptionInTemplates,
        &UsingFirstPersonInTemplates,
        &UsingQuestionInExtendedDescriptionInTemplates,
        &PostinstDoesNotLoadConfmodule,
        &PostinstUsesDbInput,
        &PostinstDoesNotCallUpdatemenus,
        &PostrmDoesNotCallUpdatemenus,
        &PostinstHasUselessCallToInstallDocs,
        &PrermHasUselessCallToInstallDocs,
        &DocBaseAbstractFieldSeparatorExtraWhitespace,
        &DocBaseFileNoFormatSection,
        &MenuMethodLacksInclude,
        &QuotedPlaceholderInMailcapEntry,
        &ReadmeDebianMentionsUsrDoc,
        &SyntaxErrorInSymbolsFile,
        &InvalidTemplateIdInSymbolsFile,
        &DuplicateEntryInSymbolsControlFile,
        &SymbolsFileContainsCurrentVersionWithDebianRevision,
        &SymbolsFileContainsDebianRevision,
        &DuplicateInShlibs,
        &MalformedMd5sumsControlFile,
        &Md5sumsListsNonexistentFile,
        &FileMissingInMd5sums,
        &Md5sumMismatch,
        &RedundantOriginField,
        &EssentialNoNotNeeded,
        &NewEssentialPackage,
        &RecommendedField,
        &UnknownSection,
        &SuperfluousClutterInHomepage,
        &BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
        &DependsOnEssentialPackageWithoutUsingVersion,
        &NonEtcFileMarkedAsConffile,
        &UsesImplicitAwaitTrigger,
        &MissingFieldInDep5Copyright,
        &MissingLicenseTextInDep5Copyright,
        &EmptyShortLicenseInDep5Copyright,
        &IncompleteCreativeCommonsLicense,
        &LicenseProblemUndefinedLicense,
        &BogusMailHostInDebianChangelog,
        &SyntaxErrorInDebianNewsFile,
        &LatestChangelogEntryWithoutNewDate,
        &DebianNewsEntryHasStrangeDistribution,
        &WrongNameForDebianNewsFile,
        &QaUploadHasIncorrectVersionNumber,
        &DebianTestsControlAutodep8IsObsolete,
        &ReadmeDebianContainsInvalidEmailAddress,
        &IllegalVariable,
        &MailAddressLoopsOrBounces,
        &FaultyDebianQaGroupAddress,
        &FaultyDebianQaGroupPhrase,
        &DebianRevisionIsZero,
        &DebianRevisionNotWellFormed,
        &UntranslatableDebconfTemplates,
        &DocBaseFileUnknownField,
        &ExcessivePriorityForLibraryPackage,
        &ExtendedDescriptionContainsEmptyParagraph,
        &FileInUsrMarkedAsConffile,
        &NodejsModuleInstalledInUsrLib,
        &GirSectionNotLibdevel,
        &HomepageForCpanPackageContainsVersion,
        &HomepageRefersToObsoleteDebianInfrastructure,
        &InfoDocumentMissingDirEntry,
        &InfoDocumentMissingDirSection,
        &InitDScriptHasBadLsbLine,
        &InvalidDateInDebianChangelog,
        &InvalidShortNameInDep5Copyright,
        &MenuItemNeedsTagHasUnknownValue,
        &MissingLicenseParagraphInDep5Copyright,
        &NeedlessSuggestRecommendLibservletJava,
        &NodePackageInstallInNodejsRootdir,
        &NodejsLockFile,
        &PossiblyInsecureHandlingOfTmpFilesInMaintainerScript,
        &RuntimeTestFileUsesInstalledPythonVersions,
        &SourceShipsExcludedFile,
        &AutotoolsPkgConfigMacroNotCrossCompilationSafe,
        &ChangelogNewsDebianMismatch,
        &MisnamedPoFile,
        &TranslatedDefaultField,
        &InvalidPoFile,
        &MissingCallToDpkgMaintscriptHelper,
        &PackageInstallsNonbinaryPerlInUsrLibPerl5,
        &UnknownFileInPythonModuleDirectory,
        &AncientLibtool,
        &Apache2ConfigurationFilesNeedConfSuffix,
        &ArchiveLibertyMismatch,
        &Apache2ReverseDependencyCallsWrapperScript,
        &Apache2UnparsableDependency,
        &PackageUsesLocalDiversion,
        &HeaderHasOverlyGenericName,
        &FileInUsrLibSgml,
        &ForbiddenConfigInterpreter,
        &Format30ButDebianChangesPatch,
        &MaintainerScriptHasUnexpandedDebhelperToken,
        &MissingDependencyOnPerlapi,
        &NoOpTestsuite,
        &PackageInstallsIntoEtcGconfSchemas,
        &PackageSectionGamesButContainsNoGame,
        &PackageSectionGamesButHasUsrBin,
        &PkgJsToolsTestIsEmpty,
        &PkgJsToolsTestIsMissing,
        &PoorCompressionInManualPage,
        &RunPartsCronFilenameContainsIllegalChars,
        &StrayTranslatedDebconfTemplates,
        &SymbolsForUndeclaredSharedLibrary,
        &UnnecessaryTestsuiteAutopkgtestField,
        &UploadersInOrphan,
        &WindowsThumbnailDatabaseInPackage,
        &MissingSystemdTimerForCronScript,
        &NonStandardApache2ConfigurationName,
        &ObsoleteCommandInModprobeDFile,
        &PackageContainsCompiledFontFile,
        &PackageInstallsIeeeData,
        &SuWrapperWithoutC,
        &UnquotedStringInMenuItem,
        &DebianControlRepeatsFieldNameInValue,
        &DocBaseFileLacksRequiredField,
        &IconSizeAndDirectoryNameMismatch,
        &MaintscriptIncludesMaintScriptParameters,
        &MenuIconUsesRelativePath,
        &MissingConffile,
        &NodejsMissingVersionOverride,
        &NonDebugFileInDebugPackage,
        &OverrideFileInWrongLocation,
        &PackageInstallsFileToUsrX11r6,
        &SuWrapperNotSuToRoot,
        &NoVersionedDebhelperPrerequisite,
        &DebhelperButNoMiscDepends,
        &CircularInstallationPrerequisite,
        &NodejsModuleNotDeclared,
        &PythonPackageMissingDependsOnPython,
        &BogusPythonPrerequisite,
        &EmbeddedFeedparserLibrary,
        &PythonModuleHasOverlyGenericName,
        &MenuIconMissing,
        &MenuItemMissingRequiredTag,
        &NonWmModuleInWmModulesMenuSection,
        &SuToRootWithUsrSbin,
        &BadTestInMenuItem,
        &DuplicateTagInMenu,
        &DesktopContainsDeprecatedKey,
        &DesktopEntryContainsDeprecatedKey,
        &DuplicateKeyInDesktop,
        &DesktopEntryUnknownType,
        &DesktopEntryMissingRequiredKey,
        &DesktopEntryLacksMainCategory,
        &DesktopMimeButNoExecCode,
        &FileInDiscouragedX11FontDirectory,
        &FileInUnknownX11FontDirectory,
        &DesktopFileInWrongDir,
        &crate::lint::rules::CopyrightContainsAutomaticallyExtractedBoilerplate,
        &crate::lint::rules::CopyrightHasUrlFromDhMakeBoilerplate,
        &crate::lint::rules::HelperTemplatesInCopyright,
        &crate::lint::rules::CopyrightWithoutCopyrightNotice,
        &crate::lint::rules::CopyrightFileContainsFullGfdlLicense,
        &crate::lint::rules::CopyrightFileContainsFullApache2License,
        &crate::lint::rules::CopyrightDoesNotReferToCommonLicenseFile,
        &crate::lint::rules::CopyrightNotUsingCommonLicenseForGfdl,
        &crate::lint::rules::DebianCopyrightIsSymlink,
        &crate::lint::rules::PipeSymbolUsedAsLicenseDisjunction,
        &crate::lint::rules::BadExceptionFormatInDep5Copyright,
        &crate::lint::rules::AmbiguousParagraphInDep5Copyright,
        &crate::lint::rules::Dep5CopyrightLicenseNameNotUnique,
        &crate::lint::rules::UnusedLicenseParagraphInDep5Copyright,
        &crate::lint::rules::Dep5FileParagraphReferencesHeaderParagraph,
        &crate::lint::rules::GlobalFilesWildcardNotFirstParagraphInDep5Copyright,
        &DebianRulesUsesInstalledPythonVersions,
        &DebianRulesSetsDpkgArchitectureVariable,
        &DebianRulesIsDhMakeTemplate,
        &DebianRulesPassesVersionInfoToDhShlibdeps,
        &DebianRulesSetsDebBuildOptions,
        &DebianRulesSetsDhCompat,
        &DebianRulesShouldNotUseDhExtraAddons,
        &DebianRulesShouldNotUseSanitizeAllBuildflag,
        &DebianRulesUpdatesControlAutomatically,
        &DebianRulesUsesDebBuildOpts,
        &BuildDependsIndepWithoutArchIndep,
        &BuildDependsArchWithoutArchDependentBinary,
        &ConflictingNegationInSourceRelation,
        &BuildDependsOnVersionedBerkeleyDb,
        &BuildDependsOnAnObsoleteJavaPackage,
        &BuildDependsOnSpecificJavaDocPackage,
        &BuildDependsOnDhSysuser,
        &BuildDependsOnNonBuildPackage,
        &BadVersionInRelation,
        &ObsoleteRelationFormInSource,
        &DbgPackageMissingDepends,
        &WeakLibraryDevDependency,
        &RedundantInstallationPrerequisite,
        &DependsOnLibdb1Compat,
        &DocPackageDependsOnMainPackage,
        &DependsOnSpecificJavaDocPackage,
        &BreaksWithoutVersion,
        &DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
        &DefaultMtaDependencyNotListedFirst,
        &BinaryPackageDependsOnToolchainPackage,
        &DocBaseAbstractFieldIsTemplate,
        &DocBaseAbstractMightContainExtraLeadingWhitespace,
        &DocBaseDocumentFieldEndsInWhitespace,
        &DocBaseDocumentFieldNotInFirstLine,
        &DocBaseFileNoFormat,
        &DocBaseFileNoIndex,
        &DocBaseFileReferencesWrongPath,
        &DocBaseFileSeparatorExtraWhitespace,
        &DocBaseFileSyntaxError,
        &DocBaseFileUnknownFormat,
        &DocBaseIndexReferencesMultipleFiles,
        &DocBaseInvalidDocumentField,
        &DocBaseUnknownSection,
        &DocBaseUsesApplicationsSection,
        &DuplicateFieldInDocBase,
        &DuplicateFormatInDocBase,
        &DhInstallmanpagesIsObsolete,
        &DhCleanKIsDeprecated,
        &DhQuiltAddonButQuiltSourceFormat,
        &DebianRulesUsesDeprecatedSystemdOverride,
        &ExcessiveDebhelperOverrides,
        &DebianRulesUsesUnnecessaryDhArgument,
        &UselessAutoreconfBuildDepends,
        &PackageUsesDebhelperButLacksBuildDepends,
        &DebhelperCompatFileContainsMultipleLevels,
        &DebhelperCompatFileIsEmpty,
        &DeclaresPossiblyConflictingDebhelperCompatVersions,
        &TemporaryDebhelperFile,
        &DhExecScriptWithoutDhExecFeatures,
        &DhExecInstallNotAllowedHere,
        &PrivacyBreachGeneric,
        &PrivacyBreachUsesEmbeddedFile,
        &PrivacyBreachGoogleAdsense,
        &PrivacyBreachDonation,
        &PrivacyBreachGoogleCse,
        &PrivacyBreachFacebook,
        &PrivacyBreachGooglePlus,
        &PrivacyBreachLogo,
        &PrivacyBreachStatisticsWebsite,
        &PrivacyBreachPiwik,
        &PrivacyBreachTwitter,
        &EmbeddedJavascriptLibrary,
        &EmbeddedPhpLibrary,
        &NonFreeFlash,
        &EmptyDebianTestsControl,
        &ConflictingTestFields,
        &BackgroundedTestCommand,
        &IllegalRuntimeTestName,
        &MissingRuntimeTestFile,
        &RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends,
        &DeclarePythonVersionsForTest,
        &DropPythonVersionDeclaration,
        &TestsuiteDependsOnObsoletePackage,
        &BooleanTemplateHasBogusDefault,
        &EmptyTranslatedChoices,
        &DuplicateLongDescriptionInTemplate,
        &DebianNewsFileNotCompressed,
        &ChangelogEmptyEntry,
        &ChangelogReferencesTempSecurityIdentifier,
        &ImprobableBugNumberInCloses,
        &DebianChangelogFileContainsObsoleteUserEmacsSettings,
        &DebianNewsEntryHasUnknownVersion,
        &HtmlChangelogWithoutTextVersion,
        &EssentialInSourcePackage,
        &ExplicitDefaultInPackageType,
        &DescriptionContainsTabs,
        &DescriptionContainsInvalidControlStatement,
        &DescriptionContainsDhMakePerlTemplate,
        &DescriptionIsDebmakeTemplate,
        &DescriptionStartsWithLeadingSpaces,
        &StaticallyLinkedBinary,
        &AlienTag,
        &MissingBuildDependencyForDhCommand,
        &DiversionForUnknownFile,
        &PatchFilePresentButNotMentionedInSeries,
        &NonStandardExecutablePerm,
        &PackageContainsHardlink,
        &ShippedFileWithoutUtf8Name,
        &MissingDepForInterpreter,
        &TruetypeFontProhibitsInstallableEmbedding,
        &ScriptWithoutInterpreter,
        &ZeroByteFileInDocDirectory,
        &ExecutableNotElfOrScript,
        &RelativeLibrarySearchPath,
        &UnusualInterpreter,
        &SharedobjectInLibraryDirectoryMissingSoname,
        &MissingBuildDependencyForDhAddon,
        &PhpScriptButNoPhpCliDep,
        &OrphanedDiversion,
        &RulesRequireBuildPrerequisite,
        &MultiarchForeignPkgconfig,
        &WrongPathForInterpreter,
        &RemoveOfUnknownDiversion,
        &ArchDependentFileNotInArchSpecificDirectory,
        &PatchSystemButNoSourceReadme,
        &DirOrFileInBuildTree,
        &ZeroByteExecutableInPath,
        &TimewarpStandardsVersion,
        &PkgConfigBadDirective,
        &PkgConfigUnavailableForCrossCompilation,
        &RootInContact,
        &PkgConfigMultiArchWrongDir,
        &RDataWithoutReadmeSource,
        &OldStyleConfigScriptMultiarchPath,
        &OpentypeFontProhibitsInstallableEmbedding,
        &OldStyleConfigScriptMultiarchPathArchAll,
        &AbsoluteSymbolicLinkTargetInSource,
        &CarriageReturnLineFeed,
        &CustomCompressionInDebianSourceOptions,
        &ArchDependentFileInUsrShare,
        &OpentypeFontWrongFilename,
        &GameOutsideSection,
        &GlobalDataInGamesDirectory,
        &BackupFileInPackage,
        &NfsTemporaryFileInPackage,
        &NmuInChangelog,
        &MaintainerUploadHasIncorrectVersionNumber,
        &BinaryNmuDebianRevisionInSource,
        &ExtraWhitespaceAroundNameInChangelogTrailer,
        &ChangelogIsSymlink,
        &InfoDocumentHasWrongExtension,
        &InfoDocumentNotCompressed,
        &InfoDocumentNotCompressedWithGzip,
        &InfoDocumentNotCompressedWithMaxCompression,
        &InfoDocumentMissingImageFile,
        &InitDScriptHasBadStartRunlevel,
        &InitDScriptHasDuplicateLsbKeyword,
        &InitDScriptHasDuplicateLsbSection,
        &InitDScriptHasUnknownLsbKeyword,
        &InitDScriptHasUnterminatedLsbSection,
        &InitDScriptMissingLsbKeyword,
        &MultiarchForeignCmakeFile,
        &MultiarchForeignSharedLibrary,
        &MultiarchForeignStaticLibrary,
        &OddStaticLibraryName,
        &IncorrectLibdirInLaFile,
        &NonEmptyDependencyLibsInLaFile,
        &AspellPackageNotArchAll,
        &DocumentationPackageNotArchitectureIndependent,
        &InvalidStandardsVersion,
        &NewerStandardsVersion,
        &MaintainerAlsoInUploaders,
        &MissingDebianSourceFormat,
        &GitPatchesNotExported,
        &InvalidDep3FormatPatchBugFieldDefaultValue,
        &InvalidDep3FormatPatchBugNotContainUrl,
        &InvalidDep3FormatPatchMaybeBugDebian,
        &InvalidDep3FormatPatchOriginFieldDefaultValue,
        &ConffileHasBadFileType,
        &DuplicateConffile,
        &FileInEtcRcDMarkedAsConffile,
        &FileInRootAndUsr,
        &LibraryInRootAndUsr,
        &DirectoryInModprobeD,
        &NonConfFileInModprobeD,
        &OldDevhelpStandard,
        &DuplicateChangelogFiles,
        &NoNewlineAtEnd,
        &CshConsideredHarmful,
        &VcsFieldBitrotted,
        &VcsFieldMismatch,
        &OrphanedPackageMaintainedInPrivateSpace,
        &OrphanedPackageNotMaintainedInDebianInfrastructure,
        &BugsFieldDoesNotReferToDebianInfrastructure,
        &OcamlDanglingCmxa,
        &OcamlDanglingCmxs,
        &PerlModuleInCoreDirectory,
        &DuplicateP3distOverride,
        &ObsoleteCommentsStyleInPhpIni,
        &MaintainerScriptCallsSystemctl,
        &MknodInMaintainerScript,
        &KillallIsDangerous,
        &SystemdDiversion,
        &AdduserWithHomeVarRun,
        &ScriptUsesUnversionedPythonInShebang,
        &InvalidValueInBuiltUsingField,
        &InvalidValueInStaticBuiltUsingField,
        &BadMenuItem,
        &BadPermissionsForEtcEmacsScript,
        &EmacsenCommonWithoutDhElpa,
        &PackageContainsMimeFileOutsidePackageDir,
        &ConfigFileReserved,
        &IncorrectNamingOfPkcs11Module,
        &PackageInstallsIntoObsoleteDir,
        &ExecutableInUsrShareDoc,
        &NestedExamplesDirectory,
        &InitDScriptMissingDependencyOnLocalFs,
        &InitDScriptMissingStart,
        &InitDScriptNotIncludedInPackage,
        &InitDScriptNotMarkedAsConffile,
        &InitDScriptProvidesVirtualFacility,
        &InitDScriptSourcingWithoutTest,
        &DuplicatePackagingFile,
        &IncorrectPackagingFilename,
        &DfsgVersionMisspelled,
        &DfsgVersionInNativePackage,
        &DuplicateContact,
        &MalformedOverride,
        &DuplicateOverrideContext,
        &OverrideFileInWrongPackage,
        &WaywardSymbolicLinkTargetInSource,
        &NativeSourceFileWithoutUtf8Name,
        &MissingCallToUpdateFonts,
        &DebianUpstreamObsoletePath,
        &ControlFileContainsDhMakeVcsComment,
        &Apache2DeprecatedAuthConfig,
        &Apache2ReverseDependencyCallsInvokeRcD,
        &Apache2UnsupportedDependency,
        &Apache2ModuleDoesNotDependOnApache2Api,
        &Apache2ModuleDoesNotShipLoadFile,
        &Apache2ReverseDependencyShipsFileInNotAllowedDirectory,
        &Apache2ReverseDependencyUsesObsoleteDirectory,
        &NonStandardApache2ModulePackageName,
        &BinaryInEtc,
        &CustomLibrarySearchPath,
        &ConfigureGeneratedFileInSource,
        &PackageContainsCmakePrivateFile,
        &BadPermissionsForEtcCronDScript,
        &PackageFileIsExecutable,
        &DhExecPrivateHelper,
        &ExecutableDebhelperFileWithoutBeingExecutable,
        &UsrShareDocSymlinkWithoutDependency,
        &ChangelogIsDhMakeTemplate,
        &WrongBugNumberInCloses,
        &ChangelogFileMissingExplicitEntry,
        &ChangelogNotCompressedWithMaxCompression,
        &MalformedDebianChangelogVersion,
        &MisspelledClosesBug,
        &NewPackageUsesDateBasedVersionNumber,
        &BoilerplateCopyrightFormatUri,
        &CommaSeparatedFilesInDep5Copyright,
        &DuplicateGlobbingPatterns,
        &FileIncludedAlready,
        &FileWithoutCopyrightInformation,
        &FilesExcludedWithoutCopyrightFormat10,
        &GlobbingPatternsOutOfOrder,
        &InvalidEscapeSequenceInDep5Copyright,
        &ObsoleteFieldInDep5Copyright,
        &PostrmDoesNotPurgeDebconf,
        &SyntaxErrorInDebconfTemplate,
        &ConfigDoesNotLoadConfmodule,
        &DebconfConfigNotExecutable,
        &DebconfIsNotARegistry,
        &IsdefaultFlagIsDeprecated,
        &MalformedTemplateName,
        &MissingDebconfDependency,
        &NoTemplateName,
        &QuiltBuildDepButNoSeriesFile,
        &ReadmeDebianContainsDebmakeTemplate,
        &VersionSubstvarForExternalPackage,
        &PackageContainsInfoDirFile,
        &WrongManualSection,
        &WrongNameForManualPage,
        &OddPlaceForManualPage,
        &ExecutableManualPage,
        &ArchWildcardInBinaryPackage,
        &MagicArchInArchList,
        &DebconfTranslationUsingGeneralList,
        &MissingFileFromPotfilesIn,
        &MissingPotfilesIn,
        &MissingTemplatesPot,
        &NotUsingPoDebconf,
        &DebianRulesShouldNotSetCFLAGSFromNoopt,
        &DebianRulesIsSymlink,
        &DebianRulesNotAMakefile,
        &DebianRulesUsesSpecialShellVariable,
        &DebianRulesUsesWrongEnvironmentVariable,
        &MissingBuildDependsForCleanTargetInDebianRules,
        &OverrideDhCleanDoesNotCallDhClean,
        &TypelibMissingGirDepends,
        &GirPackageNameDoesNotMatch,
        &TypelibNotInMultiarchDirectory,
        &TypelibPackageNameDoesNotMatch,
        &TypelibSectionNotIntrospection,
        &GirInArchAllPackage,
        &DescriptionIsDhMakeTemplate,
        &SectionIsDhMakeTemplate,
        &BadVersionNumber,
        &NoDebianChanges,
        &MissingDependsOnSensibleUtils,
        &LicenseProblemMd5sumNonFreeFile,
        &NodejsModuleInstalledInBadDirectory,
        &PkgJsAutopkgtestFileDoesNotExist,
        &RedundantBuildPrerequisites,
        &AlternatesNotAllowed,
        &BadRelation,
        &InvalidProfileNameInSourceRelation,
        &LibRecommendsDocumentation,
        &LibmoduleBuildPerlNeedsToBeInBuildDepends,
        &LibmoduleBuildTinyPerlNeedsToBeInBuildDepends,
        &NeedlessDependencyOnJre,
        &ObsoleteRelationForm,
        &ExecutableIsNotWorldReadable,
        &NonStandardSetuidExecutablePerm,
        &BadPermForFileInEtcSudoersD,
        &BadPermissionsForAliFile,
        &NonStandardDirPerm,
        &NonStandardFilePerm,
        &ScriptInEtcInitDNotRegisteredViaUpdateRcD,
        &DirectoryInEtcSvDirectoryWithoutExecutableRunScript,
        &DuplicateUpdatercDCallsInPostinst,
        &DuplicateUpdatercDCallsInPostrm,
        &InitDScriptContainsSkeletonTemplateContent,
        &InitDScriptDependsOnUnknownVirtualFacility,
        &InitDScriptUsesUsrInterpreter,
        &PackageInstallsPythonBytecode,
        &PythonModuleInWrongLocation,
        &DjangoPackageDoesNotDependOnDjango,
        &NewPackageShouldNotPackagePython2Module,
        &ExecutableInUsrShareDocbase,
        &ExecutableMenuFile,
        &MaintainerScriptDoesNotCheckForExistenceOfInstalldocs,
        &MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus,
        &PackageContainsMimeCacheFile,
        &ExecutableDesktopFile,
        &MenuIconCannotBeParsed,
        &MenuIconNotInXpmFormat,
        &MenuIconTooBig,
        &MenuItemContainsUnknownTag,
        &MenuItemCreatesNewRootSection,
        &MenuItemNeedsDwww,
        &MenuItemUsesAppsGamesSection,
        &MenuItemUsesIconNone,
        &Python3ScriptButNoPython3Dep,
        &UnusualControlInterpreter,
        &ControlInterpreterWithoutDepends,
        &MultiArchSamePackageCallsPycompile,
        &PhpScriptWithUnusualInterpreter,
        &ControlInterpreterInUsrLocal,
        &ControlInterpreterWithoutPredepends,
        &ExampleScriptUsesDeprecatedNodejsLocation,
        &ForbiddenPostrmInterpreter,
        &GawkScriptButNoGawkDep,
        &IncorrectPathForInterpreter,
        &InterpreterInUsrLocal,
        &InterpreterNotAbsolute,
        &MaintainerScriptSetsAlternativeImproperly,
        &MawkScriptButNoMawkDep,
        &SystemdServiceFileRefersToUnusualWantedbyTarget,
        &SystemdServiceFileUsesDeprecatedSyslogFacility,
        &SystemdServiceInOddLocation,
        &InitScriptIsNotAFile,
        &KillModeNone,
        &OmittedSystemdServiceForInitDScript,
        &MissingTestsControl,
        &ObsoleteRuntimeTestsRestriction,
        &ElevatedPrivileges,
        &WebApplicationDependsOnApache2DataPackage,
        &WebApplicationWorksOnlyWithApache,
        &LibappPerlPackageName,
        &PackageInstallsAptKeyring,
        &PackageInstallsAptPreferences,
        &PackageInstallsAptSources,
        &ArchIndependentPackageContainsBinaryOrObject,
        &DebugSymbolsDirectlyInUsrLibDebug,
        &DebugSymbolsNotDetached,
        &MissingDependencyOnPhpapi,
        &SourceContainsWafBinary,
        &RelativeConffile,
        &UnexpectedConffile,
        &UnknownConffileInstruction,
        &ControlFileHasBadOwner,
        &MultilineArchitectureField,
        &InvalidProfileNameInBuildProfilesField,
        &VariesContentWithNocheck,
        &DebianControlFileIsASymlink,
        &SyntaxErrorInDep5Copyright,
        &TabInLicenseText,
        &UnknownParagraphInDep5Copyright,
        &SelectWithoutChoices,
        &UnknownTemplateType,
        &DebianRulesDefinesDpkgBuildApi,
        &InvalidOverrideRestriction,
        &UnwantedPathTooSpecific,
        &UnknownEncodingInPoFile,
        &DistantPrerequisiteInShlibs,
        &EmptyShlibs,
        &NoShlibs,
        &OutdatedRelationInShlibs,
        &SharedLibraryLacksVersion,
        &SharedLibraryNotShipped,
        &SharedLibrarySymbolsNotTracked,
        &ShipsUndeclaredSharedLibrary,
        &SurplusSharedLibrarySymbols,
        &UnknownMetaFieldInSymbolsFile,
        &PackagePlaceholderInSymbolsFile,
        &RasterImageInScalableDirectory,
        &PackageContainsMultipleDpiFonts,
        &PackageMixesMiscAndDpiFonts,
        &ReadmeSourceIsDhMakeTemplate,
        &TooManyArchitectures,
        &UnknownArchitecture,
        &EmptyField,
        &UnknownEssentialValue,
        &UnsupportedSourceFormat,
        &WrongTeam,
        &FontPackageNotMultiArchForeign,
        &IllegalMultiArchValue,
        &MultiArchSamePackageHasArchSpecificOverrides,
        &UnknownMultiArchValue,
        &BadPackageName,
        &PackageNotLowercase,
        &PriorityExtraIsReplacedByPriorityOptional,
        &UnknownPriority,
        &WrongSectionForUdeb,
        &SourceFieldMalformed,
        &UploaderNameMissing,
        &TripletDirAndArchitectureMismatch,
        &SourceContainsPrebuiltMsHelpFile,
        &BrokenGz,
        &GzipFileIsNotMultiArchSameSafe,
        &PackageContainsTimestampedGzip,
        &BrokenLzma,
        &BrokenXz,
        &NonStandardFilePermissionsForEtcInitDScript,
        &PackageInstallsDeprecatedUpstartConfiguration,
        &PackageInstallsIntoEtcRcBoot,
        &PackageInstallsIntoEtcRcD,
        &PackageModifiesLdSoSearchPath,
        &MissingIntermediateDirectory,
        &PrivacyBreachW3cValidHtml,
        &ScriptWithLanguageExtension,
        &SourceContainsPrebuiltSilverlightObject,
        &SpecialFile,
        &DuplicateFontFile,
        &FontInNonFontPackage,
        &WoffFontInWrongDir,
        &TruetypeFontWrongFilename,
        &WindowsDevelFileInPackage,
        &PackageContainsThumbnailsDir,
        &PackageContainsXvpicsDir,
        &PackageNameDefinedInConfigH,
        &PostrmContainsAdditionalUpdatercDCalls,
        &PostrmDoesNotCallUpdatercDForInitDScript,
        &PreinstCallsUpdatercD,
        &PrermCallsUpdatercD,
        &UpstartJobInEtcInitDNotRegisteredViaUpdateRcD,
        &GfortranModuleDoesNotDeclareVersion,
        &MissingPrerequisiteForGfortranModule,
        &GoLibraryPackageDoesNotIncludeVersion,
        &PackageInstallsJavaBytecode,
        &PackageInstallsPacklist,
        &PackageInstallsPerllocalPod,
        &PerlModuleUsesPerl4LibsWithoutDep,
        &ScriptUsesPerl4LibsWithoutDep,
        &ComposerPrerequisite,
        &PuppetModuleButNotDhPuppet,
        &PackageInstallsPythonEgg,
        &PackageInstallsPythonPycacheDir,
        &PythonDebugInWrongLocation,
        &PypiHomepage,
        &RPackageNotArchAll,
        &RubyInterpreterIsDeprecated,
        &RubygemHomepage,
        &EmptyRustLibraryDeclaresProvides,
        &StrippedLibrary,
        &OddPermissionsOnSharedLibrary,
        &SharedLibraryIsExecutable,
        &SharedLibraryIsMultiArchForeign,
        &PackageNameDoesntMatchSonames,
        &UnconditionalUseOfDpkgStatoverride,
        &UdebPostinstCallsLdconfig,
        &PackageContainsMimeinfoCacheFile,
        &EmptyUpstreamSources,
        &PackageDependsOnHardcodedLibc,
        &DependsOnMiscPreDepends,
        &MisplacedSystemdTmpfiles,
        &TestLeavesPythonVersionUntested,
        &TestsuiteDependencyHasUnparsableElements,
        &TestsuiteRestrictionsHasDeprecatedSkipNotInstallable,
        &UnknownTestsuite,
        &UsesDeprecatedAdttmp,
        &VimAddonWithinVimRuntimePath,
        &MisplacedExtraMemberInDeb,
        &UsesNoCompressionForControlTarball,
        &UsesNoCompressionForDataTarball,
        &HardeningNoRelro,
        &HardeningNoBindnow,
        &HardeningNoPie,
        &ExecutableStackInSharedLibrary,
        &SharedLibraryLacksStackSection,
        &SpecificAddressInSharedLibrary,
        &HardeningNoFortifyFunctions,
        &BinaryCompiledWithProfilingEnabled,
        &ObsoleteDesEncryption,
        &ObsoleteCryptAlias,
        &UdebUsesUnsupportedCompressionForDataTarball,
        &BinaryWithBadDynamicTable,
        &ZipParseError,
        &BadJarName,
        &JarContainsSource,
        &UnknownJavaClassVersion,
        &IncompatibleJavaBytecodeFormat,
        &CodelessJar,
        &MissingManifest,
        &ExecutableJarWithoutMainClass,
        &MissingDepOnJarwrapper,
        &JarNotInUsrShare,
        &ClasspathContainsRelativePath,
        &MavenPluginInUsrShareJava,
        &VersionRefersToUnstableDistributionWithExpVersion,
        &VersionRefersToDistribution,
        &RcVersionGreaterThanExpectedVersion,
        &PossibleNewUpstreamReleaseWithoutNewVersion,
        &UpstreamMetadataInNativeSource,
        &UpstreamMetadataIsNotAFile,
        &TeamUploadHasIncorrectVersionNumber,
        &UnnecessaryTeamUpload,
        &ControlFileHasBadPermissions,
        &ControlFileIsEmpty,
        &ControlFileIsNotAFile,
        &NotAllowedControlFile,
        &UnknownControlFile,
        &PossibleMissingColonInCloses,
        &WrongNameForChangelogOfNativePackage,
        &WrongNameForDebianChangelogFile,
        &WrongNameForUpstreamChangelog,
        &PackageUsesVendorSpecificPatchSeries,
        &QuiltPatchWithNonStandardOptions,
        &QuiltSeriesButNoBuildDep,
        &QuiltSeriesWithoutTrailingNewline,
        &UnknownFileInDebianSource,
        &DbusPolicyAtConsole,
        &DbusPolicyExcessivelyBroad,
        &DbusPolicyInEtc,
        &DbusPolicyWithoutSendDestination,
        &DbusSystemServiceWrongName,
        &RubyScriptButNoRubyDep,
        &ScriptNotExecutable,
        &ScriptUsesDeprecatedNodejsLocation,
        &TclshScriptButNoTclshDep,
        &TrailingSlashForDpkgMaintscriptHelperSymlinkToDir,
        &UnknownControlInterpreter,
        &WishScriptButNoWishDep,
        &UdevRuleInEtc,
        &UdevRuleMissingSubsystem,
        &UdevRuleMissingUaccess,
        &UdevRuleUnreadable,
        &ServiceFileIsNotAFile,
        &SystemdServiceFileShutdownProblems,
        &RepeatedTriggerName,
        &UnknownTrigger,
        &UnparsableMenuItem,
        &WhitespaceAfterContinuationCharacter,
        &PostinstHasUselessCallToUpdateMenus,
        &PostrmCallsInstalldocs,
        &PostrmHasUselessCallToUpdateMenus,
        &PreinstCallsInstalldocs,
        &PreinstCallsUpdatemenus,
        &PrermCallsUpdatemenus,
        &BrokenZip,
        &UsrShareDocSymlinkPointsOutsideOfUsrShareDoc,
        &SymlinkTargetInBuildTree,
        &SymlinkTargetInTmp,
        &DhMakeTemplateInSource,
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
        &MalformedDmUploadAllowed,
        &WrongPackageMailAlias,
        &SourceContainsCmakeCacheFile,
        &MissingDebianWatchFileStandard,
        &ObsoleteDebianWatchFileStandard,
        &UnknownDebianWatchFileStandard,
        &MultipleDebianWatchFileStandards,
        &AncientPythonVersionField,
        &DebianWatchLineInvalid,
        &DebianWatchLacksSourceforgeRedirector,
        &DebianWatchFileUsesDeprecatedGithubredir,
        &DebianWatchFileUsesDeprecatedSfRedirectorMethod,
        &DebianWatchNotManglingVersion,
        &DebianWatchFileSpecifiesWrongUpstreamVersion,
        &DebianWatchFileSpecifiesOldUpstreamVersion,
        &DebianWatchFilePubkeyFileIsMissing,
        &DebianWatchCouldVerifyDownload,
        &MalformedPythonVersion,
        &PythonVersionCurrentIsDeprecated,
        &SourcePackageEncodesPythonVersion,
        &BogusMailHost,
        &NotBinnmuableAllDependsAny,
        &NotBinnmuableAnyDependsAny,
        &NotBinnmuableAnyDependsAll,
        &SpaceInStdShortnameInDep5Copyright,
        &UnknownField,
        &VcsFieldUsesNotRecommendedUriFormat,
        &DebhelperBaseCompatLevelIsMissing,
        &TypoInDebhelperOverrideTarget,
        &MaintainerScriptLacksDebhelperToken,
        &DebianAddsArchControlDir,
        &DebianAddsArchInventoryFile,
        &DebianAddsBtsControlDir,
        &DebianAddsBzrControlDir,
        &DebianAddsCvsConflictCopy,
        &DebianAddsCvsControlDir,
        &DebianAddsEditorBackupFile,
        &DebianAddsGitControlDir,
        &DebianAddsHgControlDir,
        &DebianAddsHgTagsFile,
        &DebianAddsPatchFailureFile,
        &DebianAddsSvkCommitFile,
        &DebianAddsSvnCommitFile,
        &DebianAddsSvnConflictFile,
        &DebianAddsSvnControlDir,
        &DebianAddsQuiltControlDir,
        &DebianRulesCallsPwd,
        &DebianRulesCallsDebhelperInOddOrder,
        &BinaryArchRulesButPkgIsArchIndep,
        &BadHomepage,
        &MissingSeparatorBetweenItems,
        &ObsoleteUrlInPackaging,
        &InvalidArchStringInSourceRelation,
        &DependsOnPackagingDev,
        &BuildDependsOnEssentialPackageWithoutUsingVersion,
        &DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
        &DebhelperToolsFromAutotoolsDevAreDeprecated,
        &DebianRulesCallsNproc,
        &OverrideDhFixpermsDoesNotCallDhFixperms,
        &BraceExpansionInDebhelperConfigFile,
        &PackageUsesDhExecButLacksBuildDepends,
        &DebianControlHasDbgsymPackage,
        &BuildPrerequisiteInInstallableSection,
        &VirtualPackageDependsWithoutRealPackageDepends,
        &BuildDependsOnBuildEssential,
        &BuildDependsOn1Revision,
        &BuildDependsOnObsoletePackage,
        &BuildDependsOnMetapackage,
        &MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
        &BuildConflictsWithBuildDependency,
        &PackageRelationWithPerlModules,
        &BuildDependsOnObsoleteBootstrap,
        &NoDebianCopyrightInSource,
        &NamedCopyrightForSingleInstallable,
        &EpochChangedButUpstreamVersionDidNotGoBackwards,
        &EpochChangeWithoutComment,
        &LatestDebianChangelogEntryReusesExistingVersion,
        &OddHistoricalDebianChangelogVersion,
        &NoNmuInChangelog,
        &SourceNmuHasIncorrectVersionNumber,
        &NoQaInChangelog,
        &TemplateUsesUnsplitChoices,
        &RedundantOriginField,
        &EssentialNoNotNeeded,
        &RecommendedField,
        &BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
        &MissingFieldInDep5Copyright,
        &MissingLicenseTextInDep5Copyright,
        &EmptyShortLicenseInDep5Copyright,
        &IncompleteCreativeCommonsLicense,
        &LicenseProblemUndefinedLicense,
        &QaUploadHasIncorrectVersionNumber,
        &DebianTestsControlAutodep8IsObsolete,
        &IllegalVariable,
        &MailAddressLoopsOrBounces,
        &FaultyDebianQaGroupAddress,
        &FaultyDebianQaGroupPhrase,
        &DebianRevisionNotWellFormed,
        &UntranslatableDebconfTemplates,
        &ExcessivePriorityForLibraryPackage,
        &HomepageForCpanPackageContainsVersion,
        &HomepageRefersToObsoleteDebianInfrastructure,
        &InvalidShortNameInDep5Copyright,
        &MissingLicenseParagraphInDep5Copyright,
        &RuntimeTestFileUsesInstalledPythonVersions,
        &SourceShipsExcludedFile,
        &AutotoolsPkgConfigMacroNotCrossCompilationSafe,
        &MisnamedPoFile,
        &TranslatedDefaultField,
        &InvalidPoFile,
        &AncientLibtool,
        &ArchiveLibertyMismatch,
        &Format30ButDebianChangesPatch,
        &NoOpTestsuite,
        &PkgJsToolsTestIsEmpty,
        &PkgJsToolsTestIsMissing,
        &StrayTranslatedDebconfTemplates,
        &UnnecessaryTestsuiteAutopkgtestField,
        &UploadersInOrphan,
        &DebianControlRepeatsFieldNameInValue,
        &MaintscriptIncludesMaintScriptParameters,
        &NoVersionedDebhelperPrerequisite,
        &DebhelperButNoMiscDepends,
        &CircularInstallationPrerequisite,
        &BogusPythonPrerequisite,
        &crate::lint::rules::DebianCopyrightIsSymlink,
        &crate::lint::rules::PipeSymbolUsedAsLicenseDisjunction,
        &crate::lint::rules::BadExceptionFormatInDep5Copyright,
        &crate::lint::rules::AmbiguousParagraphInDep5Copyright,
        &crate::lint::rules::Dep5CopyrightLicenseNameNotUnique,
        &crate::lint::rules::UnusedLicenseParagraphInDep5Copyright,
        &crate::lint::rules::Dep5FileParagraphReferencesHeaderParagraph,
        &crate::lint::rules::GlobalFilesWildcardNotFirstParagraphInDep5Copyright,
        &DebianRulesUsesInstalledPythonVersions,
        &DebianRulesSetsDpkgArchitectureVariable,
        &DebianRulesIsDhMakeTemplate,
        &DebianRulesPassesVersionInfoToDhShlibdeps,
        &DebianRulesSetsDebBuildOptions,
        &DebianRulesSetsDhCompat,
        &DebianRulesShouldNotUseDhExtraAddons,
        &DebianRulesShouldNotUseSanitizeAllBuildflag,
        &DebianRulesUpdatesControlAutomatically,
        &DebianRulesUsesDebBuildOpts,
        &BuildDependsIndepWithoutArchIndep,
        &BuildDependsArchWithoutArchDependentBinary,
        &ConflictingNegationInSourceRelation,
        &BuildDependsOnVersionedBerkeleyDb,
        &BuildDependsOnAnObsoleteJavaPackage,
        &BuildDependsOnSpecificJavaDocPackage,
        &BuildDependsOnDhSysuser,
        &BuildDependsOnNonBuildPackage,
        &BadVersionInRelation,
        &ObsoleteRelationFormInSource,
        &DbgPackageMissingDepends,
        &WeakLibraryDevDependency,
        &RedundantInstallationPrerequisite,
        &DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
        &DefaultMtaDependencyNotListedFirst,
        &DhInstallmanpagesIsObsolete,
        &DhCleanKIsDeprecated,
        &DhQuiltAddonButQuiltSourceFormat,
        &DebianRulesUsesDeprecatedSystemdOverride,
        &ExcessiveDebhelperOverrides,
        &DebianRulesUsesUnnecessaryDhArgument,
        &UselessAutoreconfBuildDepends,
        &PackageUsesDebhelperButLacksBuildDepends,
        &DebhelperCompatFileContainsMultipleLevels,
        &DebhelperCompatFileIsEmpty,
        &DeclaresPossiblyConflictingDebhelperCompatVersions,
        &TemporaryDebhelperFile,
        &DhExecScriptWithoutDhExecFeatures,
        &DhExecInstallNotAllowedHere,
        &EmptyDebianTestsControl,
        &ConflictingTestFields,
        &BackgroundedTestCommand,
        &IllegalRuntimeTestName,
        &MissingRuntimeTestFile,
        &RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends,
        &DeclarePythonVersionsForTest,
        &DropPythonVersionDeclaration,
        &TestsuiteDependsOnObsoletePackage,
        &EssentialInSourcePackage,
        &MissingBuildDependencyForDhCommand,
        &PatchFilePresentButNotMentionedInSeries,
        &MissingBuildDependencyForDhAddon,
        &RulesRequireBuildPrerequisite,
        &PatchSystemButNoSourceReadme,
        &TimewarpStandardsVersion,
        &RootInContact,
        &RDataWithoutReadmeSource,
        &AbsoluteSymbolicLinkTargetInSource,
        &CarriageReturnLineFeed,
        &CustomCompressionInDebianSourceOptions,
        &NmuInChangelog,
        &MaintainerUploadHasIncorrectVersionNumber,
        &BinaryNmuDebianRevisionInSource,
        &ExtraWhitespaceAroundNameInChangelogTrailer,
        &ChangelogIsSymlink,
        &InvalidStandardsVersion,
        &NewerStandardsVersion,
        &MaintainerAlsoInUploaders,
        &MissingDebianSourceFormat,
        &GitPatchesNotExported,
        &InvalidDep3FormatPatchBugFieldDefaultValue,
        &InvalidDep3FormatPatchBugNotContainUrl,
        &InvalidDep3FormatPatchMaybeBugDebian,
        &InvalidDep3FormatPatchOriginFieldDefaultValue,
        &NoNewlineAtEnd,
        &VcsFieldBitrotted,
        &VcsFieldMismatch,
        &OrphanedPackageMaintainedInPrivateSpace,
        &OrphanedPackageNotMaintainedInDebianInfrastructure,
        &DuplicateP3distOverride,
        &DuplicatePackagingFile,
        &IncorrectPackagingFilename,
        &DfsgVersionMisspelled,
        &DfsgVersionInNativePackage,
        &DuplicateContact,
        &MalformedOverride,
        &DuplicateOverrideContext,
        &WaywardSymbolicLinkTargetInSource,
        &NativeSourceFileWithoutUtf8Name,
        &DebianUpstreamObsoletePath,
        &ControlFileContainsDhMakeVcsComment,
        &SuperfluousClutterInHomepage,
        &ConfigureGeneratedFileInSource,
        &PackageFileIsExecutable,
        &DhExecPrivateHelper,
        &ExecutableDebhelperFileWithoutBeingExecutable,
        &ChangelogFileMissingExplicitEntry,
        &MalformedDebianChangelogVersion,
        &BoilerplateCopyrightFormatUri,
        &CommaSeparatedFilesInDep5Copyright,
        &DuplicateGlobbingPatterns,
        &FileIncludedAlready,
        &FileWithoutCopyrightInformation,
        &FilesExcludedWithoutCopyrightFormat10,
        &GlobbingPatternsOutOfOrder,
        &InvalidEscapeSequenceInDep5Copyright,
        &ObsoleteFieldInDep5Copyright,
        &SyntaxErrorInDebconfTemplate,
        &QuiltBuildDepButNoSeriesFile,
        &VersionSubstvarForExternalPackage,
        &MagicArchInArchList,
        &DebconfTranslationUsingGeneralList,
        &MissingFileFromPotfilesIn,
        &MissingPotfilesIn,
        &MissingTemplatesPot,
        &NotUsingPoDebconf,
        &DebianRulesShouldNotSetCFLAGSFromNoopt,
        &DebianRulesIsSymlink,
        &DebianRulesNotAMakefile,
        &DebianRulesUsesSpecialShellVariable,
        &DebianRulesUsesWrongEnvironmentVariable,
        &MissingBuildDependsForCleanTargetInDebianRules,
        &OverrideDhCleanDoesNotCallDhClean,
        &TypelibMissingGirDepends,
        &NoDebianChanges,
        &LicenseProblemMd5sumNonFreeFile,
        &PkgJsAutopkgtestFileDoesNotExist,
        &RedundantBuildPrerequisites,
        &InvalidProfileNameInSourceRelation,
        &LibmoduleBuildPerlNeedsToBeInBuildDepends,
        &LibmoduleBuildTinyPerlNeedsToBeInBuildDepends,
        &MissingTestsControl,
        &ObsoleteRuntimeTestsRestriction,
        &SourceContainsWafBinary,
        &MultilineArchitectureField,
        &InvalidProfileNameInBuildProfilesField,
        &VariesContentWithNocheck,
        &DebianControlFileIsASymlink,
        &SyntaxErrorInDep5Copyright,
        &TabInLicenseText,
        &UnknownParagraphInDep5Copyright,
        &DebianRulesDefinesDpkgBuildApi,
        &UnwantedPathTooSpecific,
        &UnknownEncodingInPoFile,
        &PackagePlaceholderInSymbolsFile,
        &ReadmeSourceIsDhMakeTemplate,
        &UnsupportedSourceFormat,
        &WrongTeam,
        &MultiArchSamePackageHasArchSpecificOverrides,
        &UploaderNameMissing,
        &SourceContainsPrebuiltMsHelpFile,
        &SourceContainsPrebuiltSilverlightObject,
        &GoLibraryPackageDoesNotIncludeVersion,
        &ComposerPrerequisite,
        &PuppetModuleButNotDhPuppet,
        &PypiHomepage,
        &RubygemHomepage,
        &EmptyUpstreamSources,
        &PackageDependsOnHardcodedLibc,
        &DependsOnMiscPreDepends,
        &TestLeavesPythonVersionUntested,
        &TestsuiteDependencyHasUnparsableElements,
        &TestsuiteRestrictionsHasDeprecatedSkipNotInstallable,
        &UnknownTestsuite,
        &UsesDeprecatedAdttmp,
        &VersionRefersToUnstableDistributionWithExpVersion,
        &VersionRefersToDistribution,
        &RcVersionGreaterThanExpectedVersion,
        &PossibleNewUpstreamReleaseWithoutNewVersion,
        &UpstreamMetadataInNativeSource,
        &UpstreamMetadataIsNotAFile,
        &TeamUploadHasIncorrectVersionNumber,
        &UnnecessaryTeamUpload,
        &PackageUsesVendorSpecificPatchSeries,
        &QuiltPatchWithNonStandardOptions,
        &QuiltSeriesButNoBuildDep,
        &QuiltSeriesWithoutTrailingNewline,
        &UnknownFileInDebianSource,
        &DhMakeTemplateInSource,
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
        &SymlinkContainsSpuriousSegments,
        &SymlinkIsSelfRecursive,
        &LengthySymlink,
        &DebianChangelogFileIsASymlink,
        &PackageContainsVcsControlDir,
        &DirOrFileInSrv,
        &DirOrFileInMnt,
        &ExtendedDescriptionIsEmpty,
        &DirOrFileInHome,
        &DirOrFileInTmp,
        &DirOrFileInOpt,
        &DirOrFileInVarWww,
        &DirOrFileInVarRun,
        &DirOrFileInVarLock,
        &DirOrFileInRun,
        &DirOrFileInEtcOpt,
        &DirInUsrLocal,
        &FileInUsrLocal,
        &FileDirectlyInUsrShare,
        &SubdirInUsrBin,
        &FsstndDirInUsr,
        &NonStandardDirInUsr,
        &FsstndDirInVar,
        &NonStandardDirInVar,
        &AliasedLocation,
        &UseOfCompatSymlink,
        &DescriptionSynopsisIsEmpty,
        &DescriptionStartsWithPackageName,
        &DescriptionSynopsisStartsWithArticle,
        &DescriptionTooShort,
        &DescriptionIsPkgName,
        &SynopsisTooLong,
        &ExtendedDescriptionLineTooLong,
        &SvnCommitFileInPackage,
        &AbsoluteSymlinkInTopLevelFolder,
        &FileNameEndsInWhitespace,
        &FileNameContainsWildcardCharacter,
        &RelativeSymlink,
        &SymlinkEndsWithSlash,
        &SymlinkHasDoubleSlash,
        &SymlinkHasTooManyUpSegments,
        &PackageContainsPythonDotDirectory,
        &PackageContainsPythonHypothesisExample,
        &PackageContainsSassCacheDirectory,
        &CompressedSymlinkWithWrongExt,
        &HyphenFile,
        &FileNameInPathIsNotAscii,
        &PackageContainsCompiledGlibSchema,
        &StarFile,
        &PackageContainsFileInEtcSkel,
        &PackageContainsFileInUsrShareHal,
        &PackageContainsIconCacheInGenericDir,
        &PackageContainsPythonCoverageFile,
        &PackageContainsPythonDoctreeFile,
        &PackageContainsPythonHeaderInIncorrectDirectory,
        &PackageContainsPythonTestsInGlobalNamespace,
        &PackageContainsEslintConfigFile,
        &PackageContainsNpmIgnoreFile,
        &BogusMailHost,
        &DependsOnObsoletePackage,
        &DependsOnOldEmacs,
        &OldFsfAddressInCopyrightFile,
        &CopyrightRefersToOldDirectory,
        &CopyrightRefersToDeprecatedBsdLicenseFile,
        &CopyrightRefersToCompressedLicense,
        &CopyrightRefersToIncorrectDirectory,
        &CopyrightRefersToVersionlessLicenseFile,
        &CopyrightRefersToNonexistentLicenseFile,
        &CopyrightFileCompressed,
        &CopyrightFileIsSymlink,
        &FileInEtcNotMarkedAsConffile,
        &NoDebconfConfig,
        &UnknownField,
        &MissingSystemdServiceForInitDScript,
        &InitDScriptDoesNotSourceInitFunctions,
        &NonMultiArchLibDir,
        &PackageContainsVcsControlFile,
        &PackageInstallsDeprecatedPython2Path,
        &FileInUsrLibSitePython,
        &ThirdPartyPackageInPythonDir,
        &MenuFileInUsrLib,
        &BadMenuFileName,
        &SvkCommitFileInPackage,
        &RedundantBugsField,
        &BadHomepage,
        &DescriptionContainsHomepage,
        &DescriptionSynopsisIsDuplicated,
        &FieldTooLong,
        &MaintainerScriptIgnoresErrors,
        &ReadInMaintainerScript,
        &RecursivePrivilegeChange,
        &CommandWithPathInMaintainerScript,
        &MaintainerScriptRemovesDeviceFiles,
        &PackageContainsBtsControlDir,
        &UsesDpkgDatabaseDirectly,
        &MaintainerScriptCallsInstallSgmlcatalog,
        &MaintainerScriptChangesNetbase,
        &MaintscriptCallsLdconfig,
        &MaintainerScriptCallsChownImproperly,
        &MaintainerScriptModifiesInetdConf,
        &MaintainerScriptShouldNotUseDpkgMaintscriptHelper,
        &InstallInfoUsedInMaintainerScript,
        &MaintainerScriptHidesInitFailure,
        &MaintainerScriptNeedsDependsOnGconf2,
        &MaintainerScriptNeedsDependsOnUcf,
        &MaintainerScriptNeedsDependsOnXmlCore,
        &MaintainerScriptNeedsDependsOnXfontsUtils,
        &MaintainerScriptCallsInitScriptDirectly,
        &MaintainerScriptShouldNotParseEtcPasswdOrGroup,
        &DependsOnMetapackage,
        &DependsOnPackagingDev,
        &DependsOnPythonMinimal,
        &NeedlesslyDependsOnAwk,
        &PackageDependsOnMultipleLibstdcVersions,
        &PackageContainsReadmeForOtherPlatformOrDistro,
        &CompressedDocumentation,
        &FileDirectlyInUsrShareDoc,
        &MacosDsStoreFileInPackage,
        &MacosResourceForkFileInPackage,
        &PackageInstallsAppimage,
        &DebugSuffixNotDbg,
        &PackageContainsLindaOverride,
        &InitDScriptDoesNotImplementRequiredOption,
        &InitDScriptStopsInSRunlevel,
        &InitDScriptMissingLsbSection,
        &SystemdServiceFileRefersToObsoleteTarget,
        &MissingSystemdServiceForInitDRcSScript,
        &SystemdServiceFileWrapsInitScript,
        &MaintainerScriptNeedsDependsOnAdduser,
        &MaintainerScriptNeedsDependsOnUpdateInetd,
        &MaintainerScriptCallsGconftool,
        &MaintainerScriptCallsStartStopDaemon,
        &NoManualPage,
        &UncompressedManualPage,
        &WrongCompressionInManualPage,
        &CountryInManual,
        &BuildPathInManual,
        &BrokenLinkToUndocumented,
        &VirtualPackageDependsWithoutRealPackageDepends,
        &DependsExclusivelyOnMakedev,
        &UnknownFieldInTemplates,
        &PossibleDebconfNoteAbuse,
        &MalformedQuestionInTemplates,
        &UnknownDebconfPriority,
        &UsingImperativeFormInTemplates,
        &LoadsObsoleteConfmodule,
        &MakingAssumptionsAboutInterfacesInTemplates,
        &MalformedPromptInTemplates,
        &MalformedTitleInTemplates,
        &MismatchTranslatedChoices,
        &MissingDebconfDependencyForPreinst,
        &NoDebconfTemplates,
        &MenuCommandNotInPackage,
        &CommandInMenuFileAndDesktopFile,
        &MenuItemCreatesNewSection,
        &DesktopCommandNotInPackage,
        &DesktopEntryContainsUnknownKey,
        &MenuItemUsesAppsSection,
        &MenuItemUsesWindowmanagersSection,
        &NonWmInWindowmanagerMenuSection,
        &DesktopEntryFileHasCrs,
        &DesktopEntryInvalidCategory,
        &DesktopEntryUsesReservedCategory,
        &PkgNotInPackageTest,
        &ConflictsWithDependency,
        &MailTransportAgentDependencyDoesNotSpecifyDefaultMta,
        &PackageDependsOnMultipleLibpngVersions,
        &PackageDependsOnMultipleTclVersions,
        &PackageDependsOnMultipleTkVersions,
        &PackageRelationWithPerlModules,
        &BadProvidedPackageName,
        &DependsOnObsoleteBootstrap,
        &GroffMessage,
        &EmptyManualPage,
        &UndocumentedManualPage,
        &BadSoLinkWithinManualPage,
        &BadWhatisEntry,
        &ManualPageFromTemplate,
        &ManualPageWithGenericName,
        &NoEnglishManualPage,
        &PodConversionMessage,
        &UselessWhatisEntry,
        &CopyrightFileLacksPointerToPerlLicense,
        &CopyrightContainsDhMakeTodoBoilerplate,
        &CopyrightNotUsingCommonLicenseForGpl,
        &CopyrightFileContainsFullGplLicense,
        &CopyrightNotUsingCommonLicenseForApache2,
        &CopyrightNotUsingCommonLicenseForLgpl,
        &SystemdServiceAliasWithoutExtension,
        &SystemdServiceFileRefersToObsoleteBindto,
        &SystemdServiceFileUsesNobodyOrNogroup,
        &InitDScriptHasConflictingStartStop,
        &InitDScriptHasBadStopRunlevel,
        &InitDScriptShouldAlwaysStartService,
        &NonVirtualFacilityInInitdScript,
        &InitDScriptDependsOnAllVirtualFacility,
        &InitDScriptPossibleMissingStop,
        &InitDScriptStartsInStopRunlevel,
        &PostrmRemovesAlternative,
        &ScriptUsesBinEnv,
        &MaintainerScriptShouldNotUsePiupartsVariable,
        &SystemdAlternatives,
        &MaintainerScriptHasInvalidUpdateInetdOptions,
        &UdevadmCalledWithoutGuard,
        &MaintainerScriptChangesLdSoConf,
        &MaintainerScriptUpdatesFontconfigCacheImproperly,
        &DebianChangelogHasWrongDayOfWeek,
        &DebianChangelogLineTooLong,
        &InitialUploadClosesNoBugs,
        &NoTemplateDescription,
        &NoTemplateType,
        &SelectWithBooleanChoices,
        &TooLongExtendedDescriptionInTemplates,
        &TooLongShortDescriptionInTemplates,
        &UsingFirstPersonInTemplates,
        &UsingQuestionInExtendedDescriptionInTemplates,
        &PostinstDoesNotLoadConfmodule,
        &PostinstUsesDbInput,
        &PostinstDoesNotCallUpdatemenus,
        &PostrmDoesNotCallUpdatemenus,
        &PostinstHasUselessCallToInstallDocs,
        &PrermHasUselessCallToInstallDocs,
        &DocBaseAbstractFieldSeparatorExtraWhitespace,
        &DocBaseFileNoFormatSection,
        &MenuMethodLacksInclude,
        &QuotedPlaceholderInMailcapEntry,
        &ReadmeDebianMentionsUsrDoc,
        &SyntaxErrorInSymbolsFile,
        &InvalidTemplateIdInSymbolsFile,
        &DuplicateEntryInSymbolsControlFile,
        &SymbolsFileContainsCurrentVersionWithDebianRevision,
        &SymbolsFileContainsDebianRevision,
        &DuplicateInShlibs,
        &MalformedMd5sumsControlFile,
        &Md5sumsListsNonexistentFile,
        &FileMissingInMd5sums,
        &Md5sumMismatch,
        &RedundantOriginField,
        &EssentialNoNotNeeded,
        &NewEssentialPackage,
        &UnknownSection,
        &SuperfluousClutterInHomepage,
        &DependsOnEssentialPackageWithoutUsingVersion,
        &NonEtcFileMarkedAsConffile,
        &UsesImplicitAwaitTrigger,
        &BogusMailHostInDebianChangelog,
        &SyntaxErrorInDebianNewsFile,
        &LatestChangelogEntryWithoutNewDate,
        &DebianNewsEntryHasStrangeDistribution,
        &WrongNameForDebianNewsFile,
        &ReadmeDebianContainsInvalidEmailAddress,
        &MailAddressLoopsOrBounces,
        &FaultyDebianQaGroupAddress,
        &FaultyDebianQaGroupPhrase,
        &DebianRevisionIsZero,
        &DebianRevisionNotWellFormed,
        &DocBaseFileUnknownField,
        &ExcessivePriorityForLibraryPackage,
        &ExtendedDescriptionContainsEmptyParagraph,
        &FileInUsrMarkedAsConffile,
        &NodejsModuleInstalledInUsrLib,
        &GirSectionNotLibdevel,
        &HomepageForCpanPackageContainsVersion,
        &HomepageRefersToObsoleteDebianInfrastructure,
        &InfoDocumentMissingDirEntry,
        &InfoDocumentMissingDirSection,
        &InitDScriptHasBadLsbLine,
        &InvalidDateInDebianChangelog,
        &MenuItemNeedsTagHasUnknownValue,
        &NeedlessSuggestRecommendLibservletJava,
        &NodePackageInstallInNodejsRootdir,
        &NodejsLockFile,
        &PossiblyInsecureHandlingOfTmpFilesInMaintainerScript,
        &ChangelogNewsDebianMismatch,
        &MissingCallToDpkgMaintscriptHelper,
        &PackageInstallsNonbinaryPerlInUsrLibPerl5,
        &UnknownFileInPythonModuleDirectory,
        &Apache2ConfigurationFilesNeedConfSuffix,
        &Apache2ReverseDependencyCallsWrapperScript,
        &Apache2UnparsableDependency,
        &PackageUsesLocalDiversion,
        &HeaderHasOverlyGenericName,
        &FileInUsrLibSgml,
        &ForbiddenConfigInterpreter,
        &MaintainerScriptHasUnexpandedDebhelperToken,
        &MissingDependencyOnPerlapi,
        &PackageInstallsIntoEtcGconfSchemas,
        &PackageSectionGamesButContainsNoGame,
        &PackageSectionGamesButHasUsrBin,
        &PoorCompressionInManualPage,
        &RunPartsCronFilenameContainsIllegalChars,
        &SymbolsForUndeclaredSharedLibrary,
        &WindowsThumbnailDatabaseInPackage,
        &MissingSystemdTimerForCronScript,
        &NonStandardApache2ConfigurationName,
        &ObsoleteCommandInModprobeDFile,
        &PackageContainsCompiledFontFile,
        &PackageInstallsIeeeData,
        &SuWrapperWithoutC,
        &UnquotedStringInMenuItem,
        &DocBaseFileLacksRequiredField,
        &IconSizeAndDirectoryNameMismatch,
        &MenuIconUsesRelativePath,
        &MissingConffile,
        &NodejsMissingVersionOverride,
        &NonDebugFileInDebugPackage,
        &OverrideFileInWrongLocation,
        &PackageInstallsFileToUsrX11r6,
        &SuWrapperNotSuToRoot,
        &NodejsModuleNotDeclared,
        &PythonPackageMissingDependsOnPython,
        &BogusPythonPrerequisite,
        &EmbeddedFeedparserLibrary,
        &PythonModuleHasOverlyGenericName,
        &MenuIconMissing,
        &MenuItemMissingRequiredTag,
        &NonWmModuleInWmModulesMenuSection,
        &SuToRootWithUsrSbin,
        &BadTestInMenuItem,
        &DuplicateTagInMenu,
        &DesktopContainsDeprecatedKey,
        &DesktopEntryContainsDeprecatedKey,
        &DuplicateKeyInDesktop,
        &DesktopEntryUnknownType,
        &DesktopEntryMissingRequiredKey,
        &DesktopEntryLacksMainCategory,
        &DesktopMimeButNoExecCode,
        &FileInDiscouragedX11FontDirectory,
        &FileInUnknownX11FontDirectory,
        &DesktopFileInWrongDir,
        &crate::lint::rules::CopyrightContainsAutomaticallyExtractedBoilerplate,
        &crate::lint::rules::CopyrightHasUrlFromDhMakeBoilerplate,
        &crate::lint::rules::HelperTemplatesInCopyright,
        &crate::lint::rules::CopyrightWithoutCopyrightNotice,
        &crate::lint::rules::CopyrightFileContainsFullGfdlLicense,
        &crate::lint::rules::CopyrightFileContainsFullApache2License,
        &crate::lint::rules::CopyrightDoesNotReferToCommonLicenseFile,
        &crate::lint::rules::CopyrightNotUsingCommonLicenseForGfdl,
        &BadVersionInRelation,
        &DependsOnLibdb1Compat,
        &DocPackageDependsOnMainPackage,
        &DependsOnSpecificJavaDocPackage,
        &BreaksWithoutVersion,
        &DefaultMtaDependencyDoesNotSpecifyMailTransportAgent,
        &DefaultMtaDependencyNotListedFirst,
        &BinaryPackageDependsOnToolchainPackage,
        &DocBaseAbstractFieldIsTemplate,
        &DocBaseAbstractMightContainExtraLeadingWhitespace,
        &DocBaseDocumentFieldEndsInWhitespace,
        &DocBaseDocumentFieldNotInFirstLine,
        &DocBaseFileNoFormat,
        &DocBaseFileNoIndex,
        &DocBaseFileReferencesWrongPath,
        &DocBaseFileSeparatorExtraWhitespace,
        &DocBaseFileSyntaxError,
        &DocBaseFileUnknownFormat,
        &DocBaseIndexReferencesMultipleFiles,
        &DocBaseInvalidDocumentField,
        &DocBaseUnknownSection,
        &DocBaseUsesApplicationsSection,
        &DuplicateFieldInDocBase,
        &DuplicateFormatInDocBase,
        &PrivacyBreachGeneric,
        &PrivacyBreachUsesEmbeddedFile,
        &PrivacyBreachGoogleAdsense,
        &PrivacyBreachDonation,
        &PrivacyBreachGoogleCse,
        &PrivacyBreachFacebook,
        &PrivacyBreachGooglePlus,
        &PrivacyBreachLogo,
        &PrivacyBreachStatisticsWebsite,
        &PrivacyBreachPiwik,
        &PrivacyBreachTwitter,
        &EmbeddedJavascriptLibrary,
        &EmbeddedPhpLibrary,
        &NonFreeFlash,
        &BooleanTemplateHasBogusDefault,
        &EmptyTranslatedChoices,
        &DuplicateLongDescriptionInTemplate,
        &DebianNewsFileNotCompressed,
        &ChangelogEmptyEntry,
        &ChangelogReferencesTempSecurityIdentifier,
        &ImprobableBugNumberInCloses,
        &DebianChangelogFileContainsObsoleteUserEmacsSettings,
        &DebianNewsEntryHasUnknownVersion,
        &HtmlChangelogWithoutTextVersion,
        &ExplicitDefaultInPackageType,
        &DescriptionContainsTabs,
        &DescriptionContainsInvalidControlStatement,
        &DescriptionContainsDhMakePerlTemplate,
        &DescriptionIsDebmakeTemplate,
        &DescriptionStartsWithLeadingSpaces,
        &StaticallyLinkedBinary,
        &AlienTag,
        &DiversionForUnknownFile,
        &NonStandardExecutablePerm,
        &PackageContainsHardlink,
        &ShippedFileWithoutUtf8Name,
        &MissingDepForInterpreter,
        &TruetypeFontProhibitsInstallableEmbedding,
        &ScriptWithoutInterpreter,
        &ZeroByteFileInDocDirectory,
        &ExecutableNotElfOrScript,
        &RelativeLibrarySearchPath,
        &UnusualInterpreter,
        &SharedobjectInLibraryDirectoryMissingSoname,
        &PhpScriptButNoPhpCliDep,
        &OrphanedDiversion,
        &MultiarchForeignPkgconfig,
        &WrongPathForInterpreter,
        &RemoveOfUnknownDiversion,
        &ArchDependentFileNotInArchSpecificDirectory,
        &DirOrFileInBuildTree,
        &ZeroByteExecutableInPath,
        &PkgConfigBadDirective,
        &PkgConfigUnavailableForCrossCompilation,
        &RootInContact,
        &PkgConfigMultiArchWrongDir,
        &OldStyleConfigScriptMultiarchPath,
        &OpentypeFontProhibitsInstallableEmbedding,
        &OldStyleConfigScriptMultiarchPathArchAll,
        &ArchDependentFileInUsrShare,
        &OpentypeFontWrongFilename,
        &GameOutsideSection,
        &GlobalDataInGamesDirectory,
        &BackupFileInPackage,
        &NfsTemporaryFileInPackage,
        &InfoDocumentHasWrongExtension,
        &InfoDocumentNotCompressed,
        &InfoDocumentNotCompressedWithGzip,
        &InfoDocumentNotCompressedWithMaxCompression,
        &InfoDocumentMissingImageFile,
        &InitDScriptHasBadStartRunlevel,
        &InitDScriptHasDuplicateLsbKeyword,
        &InitDScriptHasDuplicateLsbSection,
        &InitDScriptHasUnknownLsbKeyword,
        &InitDScriptHasUnterminatedLsbSection,
        &InitDScriptMissingLsbKeyword,
        &MultiarchForeignCmakeFile,
        &MultiarchForeignSharedLibrary,
        &MultiarchForeignStaticLibrary,
        &OddStaticLibraryName,
        &IncorrectLibdirInLaFile,
        &NonEmptyDependencyLibsInLaFile,
        &AspellPackageNotArchAll,
        &DocumentationPackageNotArchitectureIndependent,
        &ConffileHasBadFileType,
        &DuplicateConffile,
        &FileInEtcRcDMarkedAsConffile,
        &FileInRootAndUsr,
        &LibraryInRootAndUsr,
        &DirectoryInModprobeD,
        &NonConfFileInModprobeD,
        &OldDevhelpStandard,
        &DuplicateChangelogFiles,
        &CshConsideredHarmful,
        &BugsFieldDoesNotReferToDebianInfrastructure,
        &OcamlDanglingCmxa,
        &OcamlDanglingCmxs,
        &PerlModuleInCoreDirectory,
        &ObsoleteCommentsStyleInPhpIni,
        &MaintainerScriptCallsSystemctl,
        &MknodInMaintainerScript,
        &KillallIsDangerous,
        &SystemdDiversion,
        &AdduserWithHomeVarRun,
        &ScriptUsesUnversionedPythonInShebang,
        &InvalidValueInBuiltUsingField,
        &InvalidValueInStaticBuiltUsingField,
        &BadMenuItem,
        &BadPermissionsForEtcEmacsScript,
        &EmacsenCommonWithoutDhElpa,
        &PackageContainsMimeFileOutsidePackageDir,
        &ConfigFileReserved,
        &IncorrectNamingOfPkcs11Module,
        &PackageInstallsIntoObsoleteDir,
        &ExecutableInUsrShareDoc,
        &NestedExamplesDirectory,
        &InitDScriptMissingDependencyOnLocalFs,
        &InitDScriptMissingStart,
        &InitDScriptNotIncludedInPackage,
        &InitDScriptNotMarkedAsConffile,
        &InitDScriptProvidesVirtualFacility,
        &InitDScriptSourcingWithoutTest,
        &OverrideFileInWrongPackage,
        &MissingCallToUpdateFonts,
        &Apache2DeprecatedAuthConfig,
        &Apache2ReverseDependencyCallsInvokeRcD,
        &Apache2UnsupportedDependency,
        &Apache2ModuleDoesNotDependOnApache2Api,
        &Apache2ModuleDoesNotShipLoadFile,
        &Apache2ReverseDependencyShipsFileInNotAllowedDirectory,
        &Apache2ReverseDependencyUsesObsoleteDirectory,
        &NonStandardApache2ModulePackageName,
        &BinaryInEtc,
        &CustomLibrarySearchPath,
        &PackageContainsCmakePrivateFile,
        &BadPermissionsForEtcCronDScript,
        &UsrShareDocSymlinkWithoutDependency,
        &ChangelogIsDhMakeTemplate,
        &WrongBugNumberInCloses,
        &ChangelogNotCompressedWithMaxCompression,
        &MisspelledClosesBug,
        &NewPackageUsesDateBasedVersionNumber,
        &PostrmDoesNotPurgeDebconf,
        &SyntaxErrorInDebconfTemplate,
        &ConfigDoesNotLoadConfmodule,
        &DebconfConfigNotExecutable,
        &DebconfIsNotARegistry,
        &IsdefaultFlagIsDeprecated,
        &MalformedTemplateName,
        &MissingDebconfDependency,
        &NoTemplateName,
        &ReadmeDebianContainsDebmakeTemplate,
        &PackageContainsInfoDirFile,
        &WrongManualSection,
        &WrongNameForManualPage,
        &OddPlaceForManualPage,
        &ExecutableManualPage,
        &ArchWildcardInBinaryPackage,
        &GirPackageNameDoesNotMatch,
        &TypelibNotInMultiarchDirectory,
        &TypelibPackageNameDoesNotMatch,
        &TypelibSectionNotIntrospection,
        &GirInArchAllPackage,
        &DescriptionIsDhMakeTemplate,
        &SectionIsDhMakeTemplate,
        &BadVersionNumber,
        &MissingDependsOnSensibleUtils,
        &NodejsModuleInstalledInBadDirectory,
        &AlternatesNotAllowed,
        &BadRelation,
        &LibRecommendsDocumentation,
        &NeedlessDependencyOnJre,
        &ObsoleteRelationForm,
        &ExecutableIsNotWorldReadable,
        &NonStandardSetuidExecutablePerm,
        &BadPermForFileInEtcSudoersD,
        &BadPermissionsForAliFile,
        &NonStandardDirPerm,
        &NonStandardFilePerm,
        &ScriptInEtcInitDNotRegisteredViaUpdateRcD,
        &DirectoryInEtcSvDirectoryWithoutExecutableRunScript,
        &DuplicateUpdatercDCallsInPostinst,
        &DuplicateUpdatercDCallsInPostrm,
        &InitDScriptContainsSkeletonTemplateContent,
        &InitDScriptDependsOnUnknownVirtualFacility,
        &InitDScriptUsesUsrInterpreter,
        &PackageInstallsPythonBytecode,
        &PythonModuleInWrongLocation,
        &DjangoPackageDoesNotDependOnDjango,
        &NewPackageShouldNotPackagePython2Module,
        &ExecutableInUsrShareDocbase,
        &ExecutableMenuFile,
        &MaintainerScriptDoesNotCheckForExistenceOfInstalldocs,
        &MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus,
        &PackageContainsMimeCacheFile,
        &ExecutableDesktopFile,
        &MenuIconCannotBeParsed,
        &MenuIconNotInXpmFormat,
        &MenuIconTooBig,
        &MenuItemContainsUnknownTag,
        &MenuItemCreatesNewRootSection,
        &MenuItemNeedsDwww,
        &MenuItemUsesAppsGamesSection,
        &MenuItemUsesIconNone,
        &Python3ScriptButNoPython3Dep,
        &UnusualControlInterpreter,
        &ControlInterpreterWithoutDepends,
        &MultiArchSamePackageCallsPycompile,
        &PhpScriptWithUnusualInterpreter,
        &ControlInterpreterInUsrLocal,
        &ControlInterpreterWithoutPredepends,
        &ExampleScriptUsesDeprecatedNodejsLocation,
        &ForbiddenPostrmInterpreter,
        &GawkScriptButNoGawkDep,
        &IncorrectPathForInterpreter,
        &InterpreterInUsrLocal,
        &InterpreterNotAbsolute,
        &MaintainerScriptSetsAlternativeImproperly,
        &MawkScriptButNoMawkDep,
        &SystemdServiceFileRefersToUnusualWantedbyTarget,
        &SystemdServiceFileUsesDeprecatedSyslogFacility,
        &SystemdServiceInOddLocation,
        &InitScriptIsNotAFile,
        &KillModeNone,
        &OmittedSystemdServiceForInitDScript,
        &ElevatedPrivileges,
        &WebApplicationDependsOnApache2DataPackage,
        &WebApplicationWorksOnlyWithApache,
        &LibappPerlPackageName,
        &PackageInstallsAptKeyring,
        &PackageInstallsAptPreferences,
        &PackageInstallsAptSources,
        &ArchIndependentPackageContainsBinaryOrObject,
        &DebugSymbolsDirectlyInUsrLibDebug,
        &DebugSymbolsNotDetached,
        &MissingDependencyOnPhpapi,
        &RelativeConffile,
        &UnexpectedConffile,
        &UnknownConffileInstruction,
        &ControlFileHasBadOwner,
        &SelectWithoutChoices,
        &UnknownTemplateType,
        &InvalidOverrideRestriction,
        &DistantPrerequisiteInShlibs,
        &EmptyShlibs,
        &NoShlibs,
        &OutdatedRelationInShlibs,
        &SharedLibraryLacksVersion,
        &SharedLibraryNotShipped,
        &SharedLibrarySymbolsNotTracked,
        &ShipsUndeclaredSharedLibrary,
        &SurplusSharedLibrarySymbols,
        &UnknownMetaFieldInSymbolsFile,
        &RasterImageInScalableDirectory,
        &PackageContainsMultipleDpiFonts,
        &PackageMixesMiscAndDpiFonts,
        &TooManyArchitectures,
        &UnknownArchitecture,
        &EmptyField,
        &UnknownEssentialValue,
        &FontPackageNotMultiArchForeign,
        &IllegalMultiArchValue,
        &UnknownMultiArchValue,
        &BadPackageName,
        &PackageNotLowercase,
        &PriorityExtraIsReplacedByPriorityOptional,
        &UnknownPriority,
        &WrongSectionForUdeb,
        &SourceFieldMalformed,
        &TripletDirAndArchitectureMismatch,
        &BrokenGz,
        &GzipFileIsNotMultiArchSameSafe,
        &PackageContainsTimestampedGzip,
        &BrokenLzma,
        &BrokenXz,
        &NonStandardFilePermissionsForEtcInitDScript,
        &PackageInstallsDeprecatedUpstartConfiguration,
        &PackageInstallsIntoEtcRcBoot,
        &PackageInstallsIntoEtcRcD,
        &PackageModifiesLdSoSearchPath,
        &MissingIntermediateDirectory,
        &PrivacyBreachW3cValidHtml,
        &ScriptWithLanguageExtension,
        &SpecialFile,
        &DuplicateFontFile,
        &FontInNonFontPackage,
        &WoffFontInWrongDir,
        &TruetypeFontWrongFilename,
        &WindowsDevelFileInPackage,
        &PackageContainsThumbnailsDir,
        &PackageContainsXvpicsDir,
        &PackageNameDefinedInConfigH,
        &PostrmContainsAdditionalUpdatercDCalls,
        &PostrmDoesNotCallUpdatercDForInitDScript,
        &PreinstCallsUpdatercD,
        &PrermCallsUpdatercD,
        &UpstartJobInEtcInitDNotRegisteredViaUpdateRcD,
        &GfortranModuleDoesNotDeclareVersion,
        &MissingPrerequisiteForGfortranModule,
        &PackageInstallsJavaBytecode,
        &PackageInstallsPacklist,
        &PackageInstallsPerllocalPod,
        &PerlModuleUsesPerl4LibsWithoutDep,
        &ScriptUsesPerl4LibsWithoutDep,
        &PackageInstallsPythonEgg,
        &PackageInstallsPythonPycacheDir,
        &PythonDebugInWrongLocation,
        &RPackageNotArchAll,
        &RubyInterpreterIsDeprecated,
        &EmptyRustLibraryDeclaresProvides,
        &StrippedLibrary,
        &OddPermissionsOnSharedLibrary,
        &SharedLibraryIsExecutable,
        &SharedLibraryIsMultiArchForeign,
        &PackageNameDoesntMatchSonames,
        &UnconditionalUseOfDpkgStatoverride,
        &UdebPostinstCallsLdconfig,
        &PackageContainsMimeinfoCacheFile,
        &MisplacedSystemdTmpfiles,
        &VimAddonWithinVimRuntimePath,
        &MisplacedExtraMemberInDeb,
        &UsesNoCompressionForControlTarball,
        &UsesNoCompressionForDataTarball,
        &HardeningNoRelro,
        &HardeningNoBindnow,
        &HardeningNoPie,
        &ExecutableStackInSharedLibrary,
        &SharedLibraryLacksStackSection,
        &SpecificAddressInSharedLibrary,
        &HardeningNoFortifyFunctions,
        &BinaryCompiledWithProfilingEnabled,
        &ObsoleteDesEncryption,
        &ObsoleteCryptAlias,
        &UdebUsesUnsupportedCompressionForDataTarball,
        &BinaryWithBadDynamicTable,
        &ZipParseError,
        &BadJarName,
        &JarContainsSource,
        &UnknownJavaClassVersion,
        &IncompatibleJavaBytecodeFormat,
        &CodelessJar,
        &MissingManifest,
        &ExecutableJarWithoutMainClass,
        &MissingDepOnJarwrapper,
        &JarNotInUsrShare,
        &ClasspathContainsRelativePath,
        &MavenPluginInUsrShareJava,
        &ControlFileHasBadPermissions,
        &ControlFileIsEmpty,
        &ControlFileIsNotAFile,
        &NotAllowedControlFile,
        &UnknownControlFile,
        &PossibleMissingColonInCloses,
        &WrongNameForChangelogOfNativePackage,
        &WrongNameForDebianChangelogFile,
        &WrongNameForUpstreamChangelog,
        &DbusPolicyAtConsole,
        &DbusPolicyExcessivelyBroad,
        &DbusPolicyInEtc,
        &DbusPolicyWithoutSendDestination,
        &DbusSystemServiceWrongName,
        &RubyScriptButNoRubyDep,
        &ScriptNotExecutable,
        &ScriptUsesDeprecatedNodejsLocation,
        &TclshScriptButNoTclshDep,
        &TrailingSlashForDpkgMaintscriptHelperSymlinkToDir,
        &UnknownControlInterpreter,
        &WishScriptButNoWishDep,
        &UdevRuleInEtc,
        &UdevRuleMissingSubsystem,
        &UdevRuleMissingUaccess,
        &UdevRuleUnreadable,
        &ServiceFileIsNotAFile,
        &SystemdServiceFileShutdownProblems,
        &RepeatedTriggerName,
        &UnknownTrigger,
        &UnparsableMenuItem,
        &WhitespaceAfterContinuationCharacter,
        &PostinstHasUselessCallToUpdateMenus,
        &PostrmCallsInstalldocs,
        &PostrmHasUselessCallToUpdateMenus,
        &PreinstCallsInstalldocs,
        &PreinstCallsUpdatemenus,
        &PrermCallsUpdatemenus,
        &BrokenZip,
        &UsrShareDocSymlinkPointsOutsideOfUsrShareDoc,
        &SymlinkTargetInBuildTree,
        &SymlinkTargetInTmp,
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use regex::Regex;

    use super::*;
    use crate::lint::rule::Code;

    /// Codes of the Rules whose source file implements `trait_name`, read from
    /// `src/lint/rules/` (one `declare_rule!` per file, ADR 0027). The lists
    /// above must register exactly these, so a new Rule file needs no edit here.
    fn implemented_codes(trait_name: &str) -> BTreeSet<String> {
        let code = Regex::new(r#"code\s*=\s*"([A-Z]{2}\d{4})""#).expect("code regex");
        let implements =
            Regex::new(&format!(r"impl\s+{trait_name}\s+for\s+\w+")).expect("impl regex");
        let mut codes = BTreeSet::new();
        let mut pending = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lint/rules")];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).expect("rules directory").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let text = fs::read_to_string(&path).expect("rule source");
                    if implements.is_match(&text)
                        && let Some(found) = code.captures(&text)
                    {
                        codes.insert(found[1].to_string());
                    }
                }
            }
        }
        codes
    }

    fn registered(codes: impl Iterator<Item = Code>) -> BTreeSet<String> {
        let codes: Vec<String> = codes.map(|code| code.as_str().to_string()).collect();
        let unique: BTreeSet<String> = codes.iter().cloned().collect();
        assert_eq!(unique.len(), codes.len(), "a Rule is registered twice");
        unique
    }

    #[test]
    fn source_rules_cover_catalogued_source_tags() {
        let codes = registered(source_rules().iter().map(|rule| rule.code()));
        assert_eq!(codes, implemented_codes("SourceRule"));
    }

    #[test]
    fn binary_package_rules_cover_catalogued_binary_tags() {
        let codes = registered(binary_package_rules().iter().map(|rule| rule.code()));
        assert_eq!(codes, implemented_codes("BinaryPackageRule"));
    }

    #[test]
    fn all_rules_is_every_rule_once() {
        let all = registered(all_rules().iter().map(|rule| rule.code()));
        let mut by_trait = implemented_codes("SourceRule");
        by_trait.extend(implemented_codes("BinaryPackageRule"));
        assert_eq!(all, by_trait);
    }
}
