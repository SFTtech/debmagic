//! Rules for lintian's `libraries/shared/stack` Check.

mod ln1154_executable_stack_in_shared_library;
mod ln1155_shared_library_lacks_stack_section;

pub(crate) use ln1154_executable_stack_in_shared_library::ExecutableStackInSharedLibrary;
pub(crate) use ln1155_shared_library_lacks_stack_section::SharedLibraryLacksStackSection;
