//! Rules for lintian's `languages/fortran/gfortran` Check.

mod ln1102_gfortran_module_does_not_declare_version;
mod ln1103_missing_prerequisite_for_gfortran_module;

pub(crate) use ln1102_gfortran_module_does_not_declare_version::GfortranModuleDoesNotDeclareVersion;
pub(crate) use ln1103_missing_prerequisite_for_gfortran_module::MissingPrerequisiteForGfortranModule;
