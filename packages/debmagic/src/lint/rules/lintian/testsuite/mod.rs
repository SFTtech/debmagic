//! Rules for lintian's `testsuite` Check.
//!
//! Each Rule reads the Source tree or Source package `debian/tests` directory.
//! Each Rule emits one Tag.

mod control;
mod ln0367_debian_tests_control_autodep8_is_obsolete;
mod ln0377_runtime_test_file_uses_installed_python_versions;
mod ln0399_no_op_testsuite;
mod ln0409_unnecessary_testsuite_autopkgtest_field;
mod ln0680_empty_debian_tests_control;
mod ln0681_conflicting_test_fields;
mod ln0682_backgrounded_test_command;
mod ln0683_illegal_runtime_test_name;
mod ln0684_missing_runtime_test_file;
mod ln0685_runtime_test_file_uses_supported_python_versions_without_test_depends;
mod ln0686_declare_python_versions_for_test;
mod ln0687_drop_python_version_declaration;
mod ln0688_testsuite_depends_on_obsolete_package;
mod ln0995_missing_tests_control;
mod ln0996_obsolete_runtime_tests_restriction;
mod python;
mod sections;

pub(crate) use ln0367_debian_tests_control_autodep8_is_obsolete::DebianTestsControlAutodep8IsObsolete;
pub(crate) use ln0377_runtime_test_file_uses_installed_python_versions::RuntimeTestFileUsesInstalledPythonVersions;
pub(crate) use ln0399_no_op_testsuite::NoOpTestsuite;
pub(crate) use ln0409_unnecessary_testsuite_autopkgtest_field::UnnecessaryTestsuiteAutopkgtestField;
pub(crate) use ln0680_empty_debian_tests_control::EmptyDebianTestsControl;
pub(crate) use ln0681_conflicting_test_fields::ConflictingTestFields;
pub(crate) use ln0682_backgrounded_test_command::BackgroundedTestCommand;
pub(crate) use ln0683_illegal_runtime_test_name::IllegalRuntimeTestName;
pub(crate) use ln0684_missing_runtime_test_file::MissingRuntimeTestFile;
pub(crate) use ln0685_runtime_test_file_uses_supported_python_versions_without_test_depends::RuntimeTestFileUsesSupportedPythonVersionsWithoutTestDepends;
pub(crate) use ln0686_declare_python_versions_for_test::DeclarePythonVersionsForTest;
pub(crate) use ln0687_drop_python_version_declaration::DropPythonVersionDeclaration;
pub(crate) use ln0688_testsuite_depends_on_obsolete_package::TestsuiteDependsOnObsoletePackage;
pub(crate) use ln0995_missing_tests_control::MissingTestsControl;
pub(crate) use ln0996_obsolete_runtime_tests_restriction::ObsoleteRuntimeTestsRestriction;
