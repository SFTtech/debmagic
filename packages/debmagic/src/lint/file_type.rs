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

    /// Classify an ELF file from its headers only, without reading the whole
    /// file. Reads the ELF header, the Section header table, and the
    /// `PT_DYNAMIC` segment (for `DF_1_PIE`); that is all [`Self::from_elf`]
    /// needs. For well-formed ELF files the result equals
    /// [`Self::from_bytes`] on the file's bytes. A file whose headers are
    /// truncated or out of bounds is `None`; damage to data this does not
    /// read (symbol tables, string tables) is not detected.
    pub fn from_elf_file(file: &std::fs::File) -> Option<Self> {
        use goblin::container::{Container, Ctx, Endian};
        use goblin::elf::{Dynamic, Elf, ProgramHeader, SectionHeader, dynamic, program_header};

        let len = file.metadata().ok()?.len();

        let mut ident = [0u8; goblin::elf::header::header64::SIZEOF_EHDR];
        let read = read_at_most(file, &mut ident, 0).ok()?;
        let header = Elf::parse_header(&ident[..read]).ok()?;
        // Fails on an unknown ELF class, as a full parse does.
        let mut elf = Elf::lazy_parse(header).ok()?;
        let ctx = Ctx::new(
            if elf.is_64 {
                Container::Big
            } else {
                Container::Little
            },
            Endian::from(elf.little_endian),
        );

        let mut is_pie = false;
        if header.e_phnum != 0 {
            let table = read_table(
                file,
                len,
                header.e_phoff,
                u64::from(header.e_phnum),
                ProgramHeader::size(ctx),
            )?;
            let program_headers =
                ProgramHeader::parse(&table, 0, usize::from(header.e_phnum), ctx).ok()?;
            if let Some(dynamic_header) = program_headers
                .iter()
                .find(|program_header| program_header.p_type == program_header::PT_DYNAMIC)
            {
                let segment = if dynamic_header.p_filesz > 0 {
                    read_range(file, len, dynamic_header.p_offset, dynamic_header.p_filesz)?
                } else {
                    Vec::new()
                };
                let mut relative = dynamic_header.clone();
                relative.p_offset = 0;
                let dynamic =
                    Dynamic::parse(&segment, std::slice::from_ref(&relative), ctx).ok()?;
                is_pie =
                    dynamic.is_some_and(|dynamic| dynamic.info.flags_1 & dynamic::DF_1_PIE != 0);
            }
        }

        if header.e_shoff != 0 {
            let entry_size = SectionHeader::size(ctx);
            let mut count = u64::from(header.e_shnum);
            if count == 0 {
                // The count overflowed `e_shnum`; the null section header
                // stores it in `sh_size`.
                let first = read_range(file, len, header.e_shoff, entry_size as u64)?;
                count = SectionHeader::parse_from(&first, 0, 1, ctx)
                    .ok()?
                    .first()?
                    .sh_size;
            }
            let table = read_table(file, len, header.e_shoff, count.max(1), entry_size)?;
            elf.section_headers =
                SectionHeader::parse_from(&table, 0, usize::try_from(count).ok()?, ctx).ok()?;
        }

        elf.is_lib = elf.is_lib && !is_pie;
        Some(Self::from_elf(&elf))
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

/// Read up to `buf.len()` bytes at `offset`; fewer only at end of file.
fn read_at_most(file: &std::fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt;

    let mut filled = 0;
    while filled < buf.len() {
        match file.read_at(&mut buf[filled..], offset + filled as u64) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// `size` bytes at `offset`, or `None` if the range is not inside a file of
/// `len` bytes or cannot be read.
fn read_range(file: &std::fs::File, len: u64, offset: u64, size: u64) -> Option<Vec<u8>> {
    if offset.checked_add(size)? > len {
        return None;
    }
    let mut bytes = vec![0u8; usize::try_from(size).ok()?];
    (read_at_most(file, &mut bytes, offset).ok()? == bytes.len()).then_some(bytes)
}

/// A table of `count` entries of `entry_size` bytes at `offset`.
fn read_table(
    file: &std::fs::File,
    len: u64,
    offset: u64,
    count: u64,
    entry_size: usize,
) -> Option<Vec<u8>> {
    read_range(file, len, offset, count.checked_mul(entry_size as u64)?)
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

    /// Header-only classification must agree with a full parse.
    fn assert_header_classification_matches(path: &std::path::Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        let file = std::fs::File::open(path)?;
        assert_eq!(
            FileType::from_elf_file(&file),
            FileType::from_bytes(&bytes),
            "{}",
            path.display()
        );
        Ok(())
    }

    #[test]
    fn header_only_classification_matches_full_parse() -> anyhow::Result<()> {
        assert_header_classification_matches(&std::env::current_exe()?)?;
        for path in [
            "/bin/sh",
            "/usr/bin/env",
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/usr/lib/x86_64-linux-gnu/libc.so.6",
            "/usr/lib/x86_64-linux-gnu/libc.a",
            "/usr/lib/x86_64-linux-gnu/crt1.o",
        ] {
            let path = std::path::Path::new(path);
            if path.is_file() && std::fs::read(path)?.starts_with(b"\x7fELF") {
                assert_header_classification_matches(path)?;
            }
        }
        Ok(())
    }

    #[test]
    fn header_only_classification_rejects_truncated_elf() -> anyhow::Result<()> {
        let bytes = std::fs::read(std::env::current_exe()?)?;
        let dir = tempfile::TempDir::new()?;
        for (name, keep) in [("tiny", 4), ("header", 64), ("half", bytes.len() / 2)] {
            let path = dir.path().join(name);
            std::fs::write(&path, &bytes[..keep])?;
            assert_header_classification_matches(&path)?;
        }
        Ok(())
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
