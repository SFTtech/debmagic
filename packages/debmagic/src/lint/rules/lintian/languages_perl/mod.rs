//! Rules for lintian's `languages/perl` Check.
//!
//! Each Rule owns one Tag.

mod ln0385_package_installs_nonbinary_perl_in_usr_lib_perl5;
mod ln0818_perl_module_in_core_directory;
mod ln1107_package_installs_packlist;
mod ln1108_package_installs_perllocal_pod;
mod ln1109_perl_module_uses_perl4_libs_without_dep;
mod ln1111_script_uses_perl4_libs_without_dep;

pub(crate) use ln0385_package_installs_nonbinary_perl_in_usr_lib_perl5::PackageInstallsNonbinaryPerlInUsrLibPerl5;
pub(crate) use ln0818_perl_module_in_core_directory::PerlModuleInCoreDirectory;
pub(crate) use ln1107_package_installs_packlist::PackageInstallsPacklist;
pub(crate) use ln1108_package_installs_perllocal_pod::PackageInstallsPerllocalPod;
pub(crate) use ln1109_perl_module_uses_perl4_libs_without_dep::PerlModuleUsesPerl4LibsWithoutDep;
pub(crate) use ln1111_script_uses_perl4_libs_without_dep::ScriptUsesPerl4LibsWithoutDep;
