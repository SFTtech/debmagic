//! Rules for lintian's `testsuite` Check.
//!
//! Each Rule reads the Source tree or Source package `debian/tests` directory.
//! Each Rule emits one Tag.

mod control;
mod ln0367_debian_tests_control_autodep8_is_obsolete;
mod ln0377_runtime_test_file_uses_installed_python_versions;

pub(crate) use ln0367_debian_tests_control_autodep8_is_obsolete::DebianTestsControlAutodep8IsObsolete;
pub(crate) use ln0377_runtime_test_file_uses_installed_python_versions::RuntimeTestFileUsesInstalledPythonVersions;
