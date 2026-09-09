/// Parsed binary classification of an Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    /// ELF; `has_static_symbol_table` is whether `SHT_SYMTAB` is present.
    Elf {
        has_static_symbol_table: bool,
        executable: bool,
    },
    Pe32,
    Pe64,
    DosMz,
}

impl FileType {
    /// ELF magic or DOS/PE `MZ` prefix — worth a full parse.
    pub fn peek_classifiable(magic: &[u8]) -> bool {
        magic.starts_with(b"\x7fELF") || magic.starts_with(b"MZ")
    }

    /// Classify bytes. Unrecognized, truncated, or non-ELF/PE/MZ is `None`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x7fELF") {
            return match goblin::Object::parse(bytes) {
                Ok(goblin::Object::Elf(elf)) => Some(Self::from_elf(&elf)),
                _ => None,
            };
        }
        if bytes.starts_with(b"MZ") {
            return match goblin::Object::parse(bytes) {
                Ok(goblin::Object::PE(pe)) => Some(if pe.is_64 { Self::Pe64 } else { Self::Pe32 }),
                _ => Some(Self::DosMz),
            };
        }
        None
    }

    fn from_elf(elf: &goblin::elf::Elf<'_>) -> Self {
        Self::Elf {
            has_static_symbol_table: elf
                .section_headers
                .iter()
                .any(|header| header.sh_type == goblin::elf::section_header::SHT_SYMTAB),
            executable: elf.header.e_type == goblin::elf::header::ET_EXEC
                || (elf.header.e_type == goblin::elf::header::ET_DYN && !elf.is_lib),
        }
    }

    pub fn is_windows_binary(self) -> bool {
        matches!(self, Self::Pe32 | Self::Pe64 | Self::DosMz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_classifiable_elf_and_mz() {
        assert!(FileType::peek_classifiable(b"\x7fELF"));
        assert!(FileType::peek_classifiable(b"MZ"));
        assert!(!FileType::peek_classifiable(b"hi"));
        assert!(!FileType::peek_classifiable(&[]));
    }

    #[test]
    fn mz_without_pe_is_dos_mz() {
        assert_eq!(FileType::from_bytes(b"MZ\0\0not pe"), Some(FileType::DosMz));
    }

    #[test]
    fn truncated_elf_is_unrecognized() {
        assert_eq!(FileType::from_bytes(b"\x7fELF"), None);
    }

    #[test]
    fn plaintext_is_unrecognized() {
        assert_eq!(FileType::from_bytes(b"hello"), None);
    }

    #[test]
    fn current_exe_is_elf() -> anyhow::Result<()> {
        let bytes = std::fs::read(std::env::current_exe()?)?;
        assert!(
            matches!(
                FileType::from_bytes(&bytes),
                Some(FileType::Elf {
                    executable: true,
                    ..
                })
            ),
            "test binary should classify as an executable ELF"
        );
        Ok(())
    }
}
