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
mod shlibs;
mod symbols;

pub(crate) use ln0336_syntax_error_in_symbols_file::SyntaxErrorInSymbolsFile;
pub(crate) use ln0337_invalid_template_id_in_symbols_file::InvalidTemplateIdInSymbolsFile;
pub(crate) use ln0338_duplicate_entry_in_symbols_control_file::DuplicateEntryInSymbolsControlFile;
pub(crate) use ln0339_symbols_file_contains_current_version_with_debian_revision::SymbolsFileContainsCurrentVersionWithDebianRevision;
pub(crate) use ln0340_symbols_file_contains_debian_revision::SymbolsFileContainsDebianRevision;
pub(crate) use ln0341_duplicate_in_shlibs::DuplicateInShlibs;
