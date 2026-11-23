//! Rules for lintian's `binaries/corrupted` Check.

mod ln1162_binary_with_bad_dynamic_table;
mod ln1292_elf_error;
mod ln1460_elf_warning;

pub(crate) use ln1162_binary_with_bad_dynamic_table::BinaryWithBadDynamicTable;
pub(crate) use ln1292_elf_error::ElfError;
pub(crate) use ln1460_elf_warning::ElfWarning;
