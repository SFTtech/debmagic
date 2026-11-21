//! Symbols and shlibs control-file text from lintian's `debian/shlibs` Check.
//!
//! Each Rule owns one Tag. The grammar is the Check's. Tags that need an ELF
//! SONAME, NEEDED, RPATH, or the dynamic symbol table are not implemented.

mod lines;
mod ln0336_syntax_error_in_symbols_file;
mod ln0337_invalid_template_id_in_symbols_file;
mod ln0338_duplicate_entry_in_symbols_control_file;
mod ln0339_symbols_file_contains_current_version_with_debian_revision;
mod ln0340_symbols_file_contains_debian_revision;
mod ln0341_duplicate_in_shlibs;
mod ln0408_symbols_for_undeclared_shared_library;
mod ln1032_distant_prerequisite_in_shlibs;
mod ln1033_empty_shlibs;
mod ln1034_no_shlibs;
mod ln1035_outdated_relation_in_shlibs;
mod ln1036_shared_library_lacks_version;
mod ln1037_shared_library_not_shipped;
mod ln1038_shared_library_symbols_not_tracked;
mod ln1039_ships_undeclared_shared_library;
mod ln1040_surplus_shared_library_symbols;
mod ln1041_unknown_meta_field_in_symbols_file;
mod shlibs;
mod symbols;

pub(crate) use ln0336_syntax_error_in_symbols_file::SyntaxErrorInSymbolsFile;
pub(crate) use ln0337_invalid_template_id_in_symbols_file::InvalidTemplateIdInSymbolsFile;
pub(crate) use ln0338_duplicate_entry_in_symbols_control_file::DuplicateEntryInSymbolsControlFile;
pub(crate) use ln0339_symbols_file_contains_current_version_with_debian_revision::SymbolsFileContainsCurrentVersionWithDebianRevision;
pub(crate) use ln0340_symbols_file_contains_debian_revision::SymbolsFileContainsDebianRevision;
pub(crate) use ln0341_duplicate_in_shlibs::DuplicateInShlibs;
pub(crate) use ln0408_symbols_for_undeclared_shared_library::SymbolsForUndeclaredSharedLibrary;
pub(crate) use ln1032_distant_prerequisite_in_shlibs::DistantPrerequisiteInShlibs;
pub(crate) use ln1033_empty_shlibs::EmptyShlibs;
pub(crate) use ln1034_no_shlibs::NoShlibs;
pub(crate) use ln1035_outdated_relation_in_shlibs::OutdatedRelationInShlibs;
pub(crate) use ln1036_shared_library_lacks_version::SharedLibraryLacksVersion;
pub(crate) use ln1037_shared_library_not_shipped::SharedLibraryNotShipped;
pub(crate) use ln1038_shared_library_symbols_not_tracked::SharedLibrarySymbolsNotTracked;
pub(crate) use ln1039_ships_undeclared_shared_library::ShipsUndeclaredSharedLibrary;
pub(crate) use ln1040_surplus_shared_library_symbols::SurplusSharedLibrarySymbols;
pub(crate) use ln1041_unknown_meta_field_in_symbols_file::UnknownMetaFieldInSymbolsFile;
