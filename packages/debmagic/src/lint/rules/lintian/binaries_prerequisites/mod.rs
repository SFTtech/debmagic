//! Rules for lintian's `binaries/prerequisites` Check.

mod ln1295_missing_dependency_on_libc;
#[allow(non_snake_case)]
mod ln1296_missing_dependency_on_libstdc__;
mod ln1297_program_not_linked_against_libc;
mod ln1298_shared_library_lacks_prerequisites;
mod shared;

pub(crate) use ln1295_missing_dependency_on_libc::MissingDependencyOnLibc;
pub(crate) use ln1296_missing_dependency_on_libstdc__::MissingDependencyOnLibstdc;
pub(crate) use ln1297_program_not_linked_against_libc::ProgramNotLinkedAgainstLibc;
pub(crate) use ln1298_shared_library_lacks_prerequisites::SharedLibraryLacksPrerequisites;
