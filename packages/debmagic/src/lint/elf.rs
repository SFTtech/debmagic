//! ELF facts of an Entry: the object type as `file(1)` names it, the
//! `PT_INTERP` interpreter, the dynamic section's `NEEDED`, `SONAME`,
//! `RPATH`, and `RUNPATH`, and the hardening facts of the Program headers
//! (`GNU_RELRO`, `GNU_STACK`) and of the dynamic section (`BIND_NOW`,
//! `TEXTREL`), and the dynamic symbols.
//!
//! Lintian reads these from `readelf --all --wide` (`Lintian::Index::Elf`)
//! and the object type from `file(1)`. This module parses the ELF header, the
//! Program header table, and the `PT_DYNAMIC` segment from the Entry's bytes
//! and nothing else, so an ELF whose Section header table or symbol tables
//! are damaged or cut off still has facts (`file(1)` and `readelf` still name
//! it). File type is a different, cheaper classification every File tree
//! computes; see ADR 0047.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use goblin::container::{Container, Ctx};
use goblin::elf::{Dynamic, Elf, ProgramHeader, dynamic, header, program_header};
use goblin::options::ParseOptions;

use super::file_tree::{Entry, EntryKind, FileTree};

/// What `file(1)` calls an ELF object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElfObjectType {
    /// `ET_EXEC` ("executable"), or `ET_DYN` with `DF_1_PIE` ("pie
    /// executable"), static-pie included.
    Executable,
    /// `ET_DYN` without `DF_1_PIE` ("shared object").
    SharedObject,
    /// `ET_REL` ("relocatable").
    Relocatable,
    /// `ET_CORE`, `ET_NONE`, or an OS- or processor-specific `e_type`.
    Other,
}

/// One dynamic symbol (`readelf --dyn-syms`), as lintian's
/// `Lintian::Elf::Symbol` keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfSymbol {
    name: String,
    undefined: bool,
    version: String,
}

impl ElfSymbol {
    /// The symbol's name, without a `@VERSION` suffix.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The symbol is undefined here (`st_shndx` is `SHN_UNDEF`, `readelf`
    /// section `UND`): the object imports it.
    pub fn is_undefined(&self) -> bool {
        self.undefined
    }

    /// The version `readelf` shows for the symbol: the name of the version
    /// it needs (`GLIBC_2.2.5`) or defines. Empty for an unversioned
    /// undefined symbol, `Base` for an unversioned defined one.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// ELF facts of one Entry.
///
/// Lists keep the order of the dynamic section. A name whose string-table
/// offset is out of range is left out; other bytes are read lossily as UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfFacts {
    object_type: ElfObjectType,
    interpreter: Option<String>,
    pie: bool,
    debug: bool,
    exec_type: bool,
    relro: bool,
    stack_flags: Option<u32>,
    textrel: bool,
    bind_now: bool,
    bad_dynamic_table: bool,
    symbols: Vec<ElfSymbol>,
    needed: Vec<String>,
    soname: Vec<String>,
    rpath: Vec<String>,
    runpath: Vec<String>,
    machine: u16,
    class: ElfClass,
    endianness: ElfEndianness,
    section_names: Vec<String>,
    errors: Vec<String>,
}

/// `EI_CLASS` of an ELF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElfClass {
    /// `ELFCLASS32`.
    Elf32,
    /// `ELFCLASS64`.
    Elf64,
}

/// `EI_DATA` of an ELF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElfEndianness {
    /// `ELFDATA2LSB`.
    Little,
    /// `ELFDATA2MSB`.
    Big,
}

impl ElfFacts {
    /// Facts of `bytes`. `None` unless they start with the ELF magic and hold
    /// a whole ELF header of a known class and byte order.
    ///
    /// A Program header table that is cut off or out of bounds leaves the
    /// interpreter and the dynamic section empty; so does a `PT_DYNAMIC`
    /// segment that cannot be read. The object type comes from the header
    /// and `DT_FLAGS_1` only.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if !bytes.starts_with(b"\x7fELF") {
            return None;
        }
        let header = Elf::parse_header(bytes).ok()?;
        let container = header.container().ok()?;
        let endian = header.endianness().ok()?;
        let ctx = Ctx::new(container, endian);

        let program_headers = usize::try_from(header.e_phoff)
            .ok()
            .and_then(|offset| {
                ProgramHeader::parse(bytes, offset, usize::from(header.e_phnum), ctx).ok()
            })
            .unwrap_or_default();

        let whole = Elf::parse_with_opts(bytes, &ParseOptions::permissive()).ok();
        let mut errors = Vec::new();
        if let Some(message) = section_table_error(bytes, &header, container) {
            errors.push(format!("In ELF header: {message}"));
            errors.push("In ELF header: Section headers are not available!".to_string());
        }
        if program_headers
            .iter()
            .any(|program_header| interpreter_is_unreadable(bytes, program_header))
        {
            errors.push("In program headers: Unable to find program interpreter name".to_string());
        }

        let mut facts = Self {
            object_type: ElfObjectType::Other,
            interpreter: program_headers
                .iter()
                .find(|program_header| program_header.p_type == program_header::PT_INTERP)
                .and_then(|interp| interpreter(bytes, interp)),
            pie: false,
            debug: false,
            exec_type: header.e_type == header::ET_EXEC,
            relro: program_headers
                .iter()
                .any(|program_header| program_header.p_type == program_header::PT_GNU_RELRO),
            stack_flags: program_headers
                .iter()
                .find(|program_header| program_header.p_type == program_header::PT_GNU_STACK)
                .map(|stack| stack.p_flags),
            textrel: false,
            bind_now: false,
            bad_dynamic_table: program_headers.iter().any(|program_header| {
                program_header.p_type == program_header::PT_DYNAMIC && program_header.p_filesz == 0
            }),
            symbols: dynamic_symbols(whole.as_ref()),
            needed: Vec::new(),
            soname: Vec::new(),
            rpath: Vec::new(),
            runpath: Vec::new(),
            machine: header.e_machine,
            class: match container {
                Container::Little => ElfClass::Elf32,
                Container::Big => ElfClass::Elf64,
            },
            endianness: if endian.is_little() {
                ElfEndianness::Little
            } else {
                ElfEndianness::Big
            },
            section_names: section_names(whole.as_ref()),
            errors,
        };

        let mut is_pie = false;
        if let Ok(Some(dynamic)) = Dynamic::parse(bytes, &program_headers, ctx) {
            is_pie = dynamic.info.flags_1 & dynamic::DF_1_PIE != 0;
            // `readelf` prints any of these as `NOW`, which lintian reads as
            // a `FLAGS_1` flag.
            facts.bind_now = dynamic.info.flags & dynamic::DF_BIND_NOW != 0
                || dynamic.info.flags_1 & dynamic::DF_1_NOW != 0;
            let strings = string_table(bytes, dynamic.info.strtab, dynamic.info.strsz);
            for entry in &dynamic.dyns {
                match entry.d_tag {
                    dynamic::DT_DEBUG => {
                        facts.debug = true;
                        continue;
                    }
                    // A `DT_FLAGS` `DF_TEXTREL` is not lintian's `TEXTREL`.
                    dynamic::DT_TEXTREL => {
                        facts.textrel = true;
                        continue;
                    }
                    dynamic::DT_BIND_NOW => {
                        facts.bind_now = true;
                        continue;
                    }
                    _ => {}
                }
                let Some(name) = usize::try_from(entry.d_val)
                    .ok()
                    .and_then(|offset| string_at(strings, offset))
                else {
                    continue;
                };
                match entry.d_tag {
                    dynamic::DT_NEEDED => facts.needed.push(name),
                    dynamic::DT_SONAME => facts.soname.push(name),
                    // Like PATH; lintian splits on `:`.
                    dynamic::DT_RPATH => facts.rpath.extend(name.split(':').map(String::from)),
                    dynamic::DT_RUNPATH => {
                        facts.runpath.extend(name.split(':').map(String::from));
                    }
                    _ => {}
                }
            }
        }

        facts.pie = is_pie;
        facts.object_type = match header.e_type {
            header::ET_EXEC => ElfObjectType::Executable,
            header::ET_DYN if is_pie => ElfObjectType::Executable,
            header::ET_DYN => ElfObjectType::SharedObject,
            header::ET_REL => ElfObjectType::Relocatable,
            _ => ElfObjectType::Other,
        };
        Some(facts)
    }

    pub fn object_type(&self) -> ElfObjectType {
        self.object_type
    }

    /// `DF_1_PIE` is set in the dynamic section (`file(1)`: "pie executable").
    pub fn is_pie(&self) -> bool {
        self.pie
    }

    /// The dynamic section has a `DT_DEBUG` entry (`readelf -d`: `DEBUG`).
    pub fn has_debug_entry(&self) -> bool {
        self.debug
    }

    /// `e_type` is `ET_EXEC`: `readelf` header type `EXEC`, a fixed-address
    /// executable. A PIE is `ET_DYN`.
    pub fn is_fixed_address_executable(&self) -> bool {
        self.exec_type
    }

    /// There is a `PT_GNU_RELRO` program header (lintian's `PH{RELRO}`).
    pub fn has_relro(&self) -> bool {
        self.relro
    }

    /// Permissions of the `PT_GNU_STACK` program header as lintian writes
    /// them (`rw-`, `rwx`), `None` without that header (lintian's
    /// `PH{STACK}{flags}`).
    pub fn stack_permissions(&self) -> Option<String> {
        self.stack_flags.map(|flags| {
            [(0b100, 'r'), (0b010, 'w'), (0b001, 'x')]
                .map(|(bit, letter)| if flags & bit != 0 { letter } else { '-' })
                .into_iter()
                .collect()
        })
    }

    /// The dynamic section has a `DT_TEXTREL` entry (lintian's `TEXTREL`).
    pub fn has_textrel(&self) -> bool {
        self.textrel
    }

    /// The dynamic section binds now: `DT_BIND_NOW`, `DT_FLAGS` with
    /// `DF_BIND_NOW`, or `DT_FLAGS_1` with `DF_1_NOW` (lintian's
    /// `FLAGS_1{NOW}`).
    pub fn binds_now(&self) -> bool {
        self.bind_now
    }

    /// There is a `PT_DYNAMIC` program header, but no dynamic section in the
    /// file (`p_filesz` is zero, as when the debugging data of a binary was
    /// split off with `objcopy --only-keep-debug`): `readelf` says "There is
    /// no dynamic section in this file" and lintian's `BAD-DYNAMIC-TABLE`.
    pub fn has_bad_dynamic_table(&self) -> bool {
        self.bad_dynamic_table
    }

    /// The dynamic symbols in table order, without the empty-named null
    /// symbol. Empty when `readelf --dyn-syms` would show none, and when the
    /// section headers, which carry the symbol versions, cannot be parsed.
    /// An archive member of a static library is not an ELF Entry, so its
    /// symbols are not here.
    pub fn symbols(&self) -> &[ElfSymbol] {
        &self.symbols
    }

    /// `PT_INTERP` up to its first NUL. `None` without that segment, or when
    /// it is empty or out of bounds (`readelf` then prints no "Requesting
    /// program interpreter").
    pub fn interpreter(&self) -> Option<&str> {
        self.interpreter.as_deref()
    }

    /// `DT_NEEDED` names. Empty is lintian's absent `NEEDED`.
    pub fn needed(&self) -> &[String] {
        &self.needed
    }

    /// `DT_SONAME` names (normally at most one).
    pub fn soname(&self) -> &[String] {
        &self.soname
    }

    /// `DT_RPATH` components, split on `:`.
    pub fn rpath(&self) -> &[String] {
        &self.rpath
    }

    /// `DT_RUNPATH` components, split on `:`.
    pub fn runpath(&self) -> &[String] {
        &self.runpath
    }

    /// Names of the Section headers in table order, the null section's empty
    /// name included (`readelf -S`: `Name`). Empty when the Section header
    /// table cannot be read; see [`ElfFacts::errors`].
    #[allow(dead_code)] // seam API; not every Rule reads it yet
    pub fn section_names(&self) -> &[String] {
        &self.section_names
    }

    /// `e_machine`, the processor the object is for (an `EM_*` value, such
    /// as 62 for `EM_X86_64`).
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn machine(&self) -> u16 {
        self.machine
    }

    /// `EI_CLASS`: 32-bit or 64-bit objects.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn class(&self) -> ElfClass {
        self.class
    }

    /// `EI_DATA`: the byte order of the object.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn endianness(&self) -> ElfEndianness {
        self.endianness
    }

    /// The error lines lintian's ELF parser keeps from `readelf` (its
    /// `ERRORS`), each prefixed with the `readelf` paragraph it came from
    /// (`In ELF header: `, `In program headers: `), in the order `readelf`
    /// prints them. Only the conditions this module can see are reported:
    /// a Section header table that runs past the end of the file, and a
    /// `PT_INTERP` segment that is empty or not in the file. `readelf` warnings
    /// (lintian's `WARNINGS`) are not modelled.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

/// The `readelf` message for a Section header table that runs past the end
/// of the file, `None` when the table is absent or fits. The size is
/// `e_shnum` times the size of one Section header of the class.
fn section_table_error(
    bytes: &[u8],
    header: &header::Header,
    container: Container,
) -> Option<String> {
    if header.e_shoff == 0 || header.e_shnum == 0 {
        return None;
    }
    let entry_size: usize = match container {
        Container::Little => 40,
        Container::Big => 64,
    };
    let size = usize::from(header.e_shnum) * entry_size;
    let fits = usize::try_from(header.e_shoff)
        .ok()
        .and_then(|offset| offset.checked_add(size))
        .is_some_and(|end| end <= bytes.len());
    (!fits).then(|| format!("Reading {size} bytes extends past end of file for section headers"))
}

/// A `PT_INTERP` segment that `readelf` cannot name the interpreter of: its
/// `p_filesz` is zero (a split-off debug file keeps the header with no
/// contents), or its bytes are not in the file.
fn interpreter_is_unreadable(bytes: &[u8], interp: &ProgramHeader) -> bool {
    if interp.p_type != program_header::PT_INTERP {
        return false;
    }
    if interp.p_filesz == 0 {
        return true;
    }
    let in_file = usize::try_from(interp.p_offset)
        .ok()
        .zip(usize::try_from(interp.p_filesz).ok())
        .and_then(|(offset, size)| offset.checked_add(size))
        .is_some_and(|end| end <= bytes.len());
    !in_file
}

/// Section names, in table order, from a permissive whole-file parse.
fn section_names(elf: Option<&Elf>) -> Vec<String> {
    let Some(elf) = elf else {
        return Vec::new();
    };
    elf.section_headers
        .iter()
        .map(|section| {
            elf.shdr_strtab
                .get_at(section.sh_name)
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

/// Dynamic symbols with their versions, from a permissive whole-file parse.
fn dynamic_symbols(elf: Option<&Elf>) -> Vec<ElfSymbol> {
    let Some(elf) = elf else {
        return Vec::new();
    };
    let version_name = |version: u16| -> Option<String> {
        if let Some(verneed) = &elf.verneed {
            for need in verneed.iter() {
                for aux in need.iter() {
                    if aux.vna_other & 0x7fff == version {
                        return elf.dynstrtab.get_at(aux.vna_name).map(String::from);
                    }
                }
            }
        }
        if let Some(verdef) = &elf.verdef {
            for definition in verdef.iter() {
                if definition.vd_ndx & 0x7fff == version {
                    let aux = definition.iter().next()?;
                    return elf.dynstrtab.get_at(aux.vda_name).map(String::from);
                }
            }
        }
        None
    };
    elf.dynsyms
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            let name = elf.dynstrtab.get_at(symbol.st_name)?;
            if name.is_empty() {
                return None;
            }
            let undefined = symbol.st_shndx == 0;
            let version = match elf.versym.as_ref().and_then(|versym| versym.get_at(index)) {
                None => String::new(),
                Some(entry) if entry.is_local() || entry.is_global() => {
                    if undefined { "" } else { "Base" }.to_string()
                }
                Some(entry) => version_name(entry.version()).unwrap_or_default(),
            };
            Some(ElfSymbol {
                name: name.to_string(),
                undefined,
                version,
            })
        })
        .collect()
}

fn interpreter(bytes: &[u8], interp: &ProgramHeader) -> Option<String> {
    let offset = usize::try_from(interp.p_offset).ok()?;
    let size = usize::try_from(interp.p_filesz).ok()?;
    let segment = bytes.get(offset..offset.checked_add(size)?)?;
    let name = segment.split(|&byte| byte == 0).next().unwrap_or_default();
    (!name.is_empty()).then(|| String::from_utf8_lossy(name).into_owned())
}

/// The dynamic string table, clipped to the file.
fn string_table(bytes: &[u8], offset: usize, size: usize) -> &[u8] {
    let start = offset.min(bytes.len());
    let end = offset.saturating_add(size).min(bytes.len());
    &bytes[start..end]
}

fn string_at(strings: &[u8], offset: usize) -> Option<String> {
    let rest = strings.get(offset..)?;
    let name = rest.split(|&byte| byte == 0).next().unwrap_or_default();
    Some(String::from_utf8_lossy(name).into_owned())
}

/// ELF facts of every regular file and hardlink in a File tree whose bytes
/// parse as ELF ([`ElfFacts::from_bytes`]), by path.
#[derive(Debug, Default)]
pub struct ElfFiles {
    facts: BTreeMap<PathBuf, ElfFacts>,
}

impl ElfFiles {
    /// Entries that cannot be read are left out, as for any other non-ELF.
    pub fn from_tree(files: &FileTree) -> Self {
        let facts = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter_map(|entry| {
                let facts = ElfFacts::from_bytes(&entry_bytes(files, entry)?)?;
                Some((entry.path().to_path_buf(), facts))
            })
            .collect();
        Self { facts }
    }

    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&ElfFacts> {
        self.facts.get(path.as_ref())
    }

    /// In path order.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &ElfFacts)> {
        self.facts
            .iter()
            .map(|(path, facts)| (path.as_path(), facts))
    }
}

/// Bytes of a file Entry, borrowed when the tree holds them in memory, so
/// non-ELF files are not copied just to look at their magic.
pub(crate) fn entry_bytes<'a>(files: &FileTree, entry: &'a Entry) -> Option<Cow<'a, [u8]>> {
    match entry {
        Entry::RegularFile {
            content: Some(bytes),
            ..
        }
        | Entry::Hardlink {
            content: Some(bytes),
            ..
        } => Some(Cow::Borrowed(bytes)),
        _ => files.read(entry).ok().map(Cow::Owned),
    }
}

/// An ELF header of `class` and byte order for `machine` with no program
/// or section headers: enough for the class, byte order and machine facts.
#[cfg(test)]
pub(crate) fn header_only(class: ElfClass, endianness: ElfEndianness, machine: u16) -> Vec<u8> {
    let (len, bits) = match class {
        ElfClass::Elf32 => (52, 1u8),
        ElfClass::Elf64 => (64, 2u8),
    };
    let little = endianness == ElfEndianness::Little;
    let mut out = vec![0u8; len];
    out[..4].copy_from_slice(b"\x7fELF");
    out[4] = bits;
    out[5] = if little { 1 } else { 2 };
    out[6] = 1;
    let put16 = |out: &mut Vec<u8>, at: usize, value: u16| {
        let bytes = if little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        out[at..at + 2].copy_from_slice(&bytes);
    };
    put16(&mut out, 16, header::ET_EXEC);
    put16(&mut out, 18, machine);
    let ehsize = if class == ElfClass::Elf32 { 40 } else { 52 };
    put16(&mut out, ehsize, len as u16);
    out
}

/// Builds small, well-formed 64-bit little-endian ELF files for tests.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct TestElf {
    e_type: u16,
    pie: bool,
    interpreter: Option<String>,
    needed: Vec<String>,
    soname: Option<String>,
    rpath: Option<String>,
    runpath: Option<String>,
    debug: bool,
    relro: bool,
    stack_flags: Option<u32>,
    textrel: bool,
    bind_now: bool,
    flags_1_now: bool,
    /// `PT_DYNAMIC` with no content in the file, as in a split-off debug file.
    no_dynamic_content: bool,
    /// Imported dynamic symbols and the version each needs.
    imports: Vec<(String, Option<String>)>,
    /// Point `e_shoff` past the end of the file, as a cut-off ELF does.
    missing_section_headers: bool,
    /// Extra empty sections with these names (`.note`, `.comment`, ...).
    sections: Vec<String>,
}

#[cfg(test)]
impl TestElf {
    fn new(e_type: u16) -> Self {
        Self {
            e_type,
            pie: false,
            interpreter: None,
            needed: Vec::new(),
            soname: None,
            rpath: None,
            runpath: None,
            debug: false,
            relro: false,
            stack_flags: None,
            textrel: false,
            bind_now: false,
            flags_1_now: false,
            no_dynamic_content: false,
            imports: Vec::new(),
            missing_section_headers: false,
            sections: Vec::new(),
        }
    }

    /// `ET_EXEC` without a dynamic section: a static executable.
    pub fn executable() -> Self {
        Self::new(header::ET_EXEC)
    }

    /// `ET_DYN` with `DF_1_PIE`: a PIE (static-pie without NEEDED).
    pub fn pie_executable() -> Self {
        Self {
            pie: true,
            ..Self::new(header::ET_DYN)
        }
    }

    /// `ET_DYN` without `DF_1_PIE`.
    pub fn shared_object() -> Self {
        Self::new(header::ET_DYN)
    }

    /// `ET_REL`.
    pub fn relocatable() -> Self {
        Self::new(header::ET_REL)
    }

    pub fn interpreter(mut self, interpreter: &str) -> Self {
        self.interpreter = Some(interpreter.to_string());
        self
    }

    pub fn needed(mut self, name: &str) -> Self {
        self.needed.push(name.to_string());
        self
    }

    pub fn soname(mut self, name: &str) -> Self {
        self.soname = Some(name.to_string());
        self
    }

    pub fn rpath(mut self, path: &str) -> Self {
        self.rpath = Some(path.to_string());
        self
    }

    pub fn runpath(mut self, path: &str) -> Self {
        self.runpath = Some(path.to_string());
        self
    }

    /// Adds `DT_DEBUG`, as glibc links executables.
    pub fn debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Adds a `PT_GNU_RELRO` program header.
    pub fn relro(mut self) -> Self {
        self.relro = true;
        self
    }

    /// Adds a `PT_GNU_STACK` program header with these `p_flags` (`6` is
    /// `RW`, `7` is `RWE`).
    pub fn stack(mut self, flags: u32) -> Self {
        self.stack_flags = Some(flags);
        self
    }

    /// Adds a `DT_TEXTREL` entry.
    pub fn textrel(mut self) -> Self {
        self.textrel = true;
        self
    }

    /// Adds `DT_FLAGS` with `DF_BIND_NOW`.
    pub fn bind_now(mut self) -> Self {
        self.bind_now = true;
        self
    }

    /// Adds `DF_1_NOW` to `DT_FLAGS_1`, as `-z now` does.
    pub fn flags_1_now(mut self) -> Self {
        self.flags_1_now = true;
        self
    }

    /// A `PT_DYNAMIC` program header whose `p_filesz` is zero.
    pub fn no_dynamic_content(mut self) -> Self {
        self.no_dynamic_content = true;
        self
    }

    /// Imports `name` (an undefined dynamic symbol), needing `version` from
    /// `libc.so.6` when given. Adds a `DT_HASH`, the dynamic symbol table,
    /// and the version sections with their Section headers.
    pub fn import(mut self, name: &str, version: Option<&str>) -> Self {
        self.imports
            .push((name.to_string(), version.map(String::from)));
        self
    }

    pub fn missing_section_headers(mut self) -> Self {
        self.missing_section_headers = true;
        self
    }

    /// Adds an empty section named `name`, after the dynamic symbol sections
    /// and before `.shstrtab`. Sections are written in the order added.
    #[allow(dead_code)] // used by the seam tests below and the Rules that follow
    pub fn section(mut self, name: &str) -> Self {
        self.sections.push(name.to_string());
        self
    }

    /// One `PT_LOAD` maps the whole file at address 0, so addresses in the
    /// dynamic section equal file offsets.
    pub fn build(&self) -> Vec<u8> {
        const EHDR: usize = 64;
        const PHDR: usize = 56;
        const SHDR: usize = 64;
        const SYM: usize = 24;

        let mut strings = vec![0u8];
        let mut add_string = |text: &str| {
            let offset = strings.len() as u64;
            strings.extend_from_slice(text.as_bytes());
            strings.push(0);
            offset
        };
        let mut dyns: Vec<(u64, u64)> = Vec::new();
        for name in &self.needed {
            dyns.push((dynamic::DT_NEEDED, add_string(name)));
        }
        if let Some(name) = &self.soname {
            dyns.push((dynamic::DT_SONAME, add_string(name)));
        }
        if let Some(path) = &self.rpath {
            dyns.push((dynamic::DT_RPATH, add_string(path)));
        }
        if let Some(path) = &self.runpath {
            dyns.push((dynamic::DT_RUNPATH, add_string(path)));
        }
        if self.pie || self.flags_1_now {
            let pie = if self.pie { dynamic::DF_1_PIE } else { 0 };
            let now = if self.flags_1_now {
                dynamic::DF_1_NOW
            } else {
                0
            };
            dyns.push((dynamic::DT_FLAGS_1, pie | now));
        }
        if self.debug {
            dyns.push((dynamic::DT_DEBUG, 0));
        }
        if self.textrel {
            dyns.push((dynamic::DT_TEXTREL, 0));
        }
        if self.bind_now {
            dyns.push((dynamic::DT_FLAGS, dynamic::DF_BIND_NOW));
        }

        // Symbol names, and the distinct versions in order of first need.
        let symbol_names: Vec<u64> = self
            .imports
            .iter()
            .map(|(name, _)| add_string(name))
            .collect();
        let mut versions: Vec<(&str, u64)> = Vec::new();
        let mut version_index: Vec<u16> = Vec::new();
        for (_, version) in &self.imports {
            let Some(version) = version else {
                version_index.push(1);
                continue;
            };
            let position = match versions.iter().position(|(known, _)| known == version) {
                Some(position) => position,
                None => {
                    versions.push((version, add_string(version)));
                    versions.len() - 1
                }
            };
            version_index.push(2 + position as u16);
        }
        let libc_name = add_string("libc.so.6");
        let has_symbols = !self.imports.is_empty();
        let has_dynamic = !dyns.is_empty() || has_symbols || self.no_dynamic_content;
        let has_section_headers = has_symbols || !self.sections.is_empty();

        // Section names: the null section, the dynamic symbol sections, the
        // extra sections, and `.shstrtab` last.
        let mut section_names = vec![String::new()];
        if has_symbols {
            section_names
                .extend([".dynsym", ".dynstr", ".gnu.version", ".gnu.version_r"].map(String::from));
        }
        section_names.extend(self.sections.iter().cloned());
        section_names.push(".shstrtab".to_string());
        let mut shstrtab = vec![0u8];
        let name_offsets: Vec<u32> = section_names
            .iter()
            .map(|name| {
                if name.is_empty() {
                    return 0;
                }
                let offset = shstrtab.len() as u32;
                shstrtab.extend_from_slice(name.as_bytes());
                shstrtab.push(0);
                offset
            })
            .collect();

        let phnum = 1
            + usize::from(self.interpreter.is_some())
            + usize::from(has_dynamic)
            + usize::from(self.relro)
            + usize::from(self.stack_flags.is_some());
        let interp_offset = EHDR + phnum * PHDR;
        let interp = self
            .interpreter
            .as_ref()
            .map(|name| {
                let mut bytes = name.as_bytes().to_vec();
                bytes.push(0);
                bytes
            })
            .unwrap_or_default();
        let strtab_offset = interp_offset + interp.len();
        let mut end = strtab_offset + strings.len();

        // Tail: dynsym, hash, versym, verneed (when there are imports).
        let nsyms = self.imports.len() + 1;
        let dynsym_offset = end.next_multiple_of(8);
        let hash_offset = dynsym_offset + nsyms * SYM;
        let versym_offset = hash_offset + 4 * (3 + nsyms);
        let verneed_offset = (versym_offset + 2 * nsyms).next_multiple_of(8);
        let verneed_size = if versions.is_empty() {
            0
        } else {
            16 + 16 * versions.len()
        };
        if has_symbols {
            end = verneed_offset + verneed_size;
        }
        let dynamic_offset = end.next_multiple_of(8);
        if has_dynamic {
            dyns.push((dynamic::DT_STRTAB, strtab_offset as u64));
            dyns.push((dynamic::DT_STRSZ, strings.len() as u64));
            if has_symbols {
                dyns.push((dynamic::DT_SYMTAB, dynsym_offset as u64));
                dyns.push((dynamic::DT_SYMENT, SYM as u64));
                dyns.push((dynamic::DT_HASH, hash_offset as u64));
            }
            dyns.push((dynamic::DT_NULL, 0));
        }
        let dynamic_size = dyns.len() * 16;
        let shstr_offset = dynamic_offset + dynamic_size;
        let shdr_offset = (shstr_offset + shstrtab.len()).next_multiple_of(8);
        let shnum: usize = if has_section_headers {
            section_names.len()
        } else {
            0
        };
        let total = if has_section_headers {
            shdr_offset + shnum * SHDR
        } else {
            dynamic_offset + dynamic_size
        };

        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"\x7fELF");
        out.extend_from_slice(&[2, 1, 1, 0]);
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&self.e_type.to_le_bytes());
        out.extend_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes()); // e_entry
        out.extend_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
        let (shoff, shnum, shstrndx): (u64, u16, u16) = if self.missing_section_headers {
            (total as u64 + 4096, 8, 0)
        } else if has_section_headers {
            (shdr_offset as u64, shnum as u16, (shnum - 1) as u16)
        } else {
            (0, 0, 0)
        };
        out.extend_from_slice(&shoff.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // e_flags
        out.extend_from_slice(&(EHDR as u16).to_le_bytes());
        out.extend_from_slice(&(PHDR as u16).to_le_bytes());
        out.extend_from_slice(&(phnum as u16).to_le_bytes());
        out.extend_from_slice(&(SHDR as u16).to_le_bytes()); // e_shentsize
        out.extend_from_slice(&shnum.to_le_bytes());
        out.extend_from_slice(&shstrndx.to_le_bytes()); // e_shstrndx

        let mut program_header = |p_type: u32, flags: u32, offset: usize, size: usize| {
            out.extend_from_slice(&p_type.to_le_bytes());
            out.extend_from_slice(&flags.to_le_bytes());
            for value in [offset, offset, offset, size, size, 8] {
                out.extend_from_slice(&(value as u64).to_le_bytes());
            }
        };
        program_header(program_header::PT_LOAD, 5, 0, total);
        if self.interpreter.is_some() {
            program_header(program_header::PT_INTERP, 4, interp_offset, interp.len());
        }
        if has_dynamic {
            let size = if self.no_dynamic_content {
                0
            } else {
                dynamic_size
            };
            program_header(program_header::PT_DYNAMIC, 6, dynamic_offset, size);
        }
        if self.relro {
            program_header(program_header::PT_GNU_RELRO, 4, 0, 0);
        }
        if let Some(flags) = self.stack_flags {
            program_header(program_header::PT_GNU_STACK, flags, 0, 0);
        }
        out.extend_from_slice(&interp);
        out.extend_from_slice(&strings);
        if has_symbols {
            out.resize(dynsym_offset, 0);
            out.extend_from_slice(&[0; SYM]); // the null symbol
            for name in &symbol_names {
                out.extend_from_slice(&(*name as u32).to_le_bytes()); // st_name
                out.push(0x12); // STB_GLOBAL, STT_FUNC
                out.push(0); // st_other
                out.extend_from_slice(&0u16.to_le_bytes()); // SHN_UNDEF
                out.extend_from_slice(&0u64.to_le_bytes()); // st_value
                out.extend_from_slice(&0u64.to_le_bytes()); // st_size
            }
            // DT_HASH: one bucket, every symbol on the chain's end.
            out.extend_from_slice(&1u32.to_le_bytes());
            out.extend_from_slice(&(nsyms as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&vec![0u8; 4 * nsyms]);
            out.extend_from_slice(&0u16.to_le_bytes()); // versym[0]: local
            for index in &version_index {
                out.extend_from_slice(&index.to_le_bytes());
            }
            out.resize(verneed_offset, 0);
            if !versions.is_empty() {
                out.extend_from_slice(&1u16.to_le_bytes()); // vn_version
                out.extend_from_slice(&(versions.len() as u16).to_le_bytes());
                out.extend_from_slice(&(libc_name as u32).to_le_bytes());
                out.extend_from_slice(&16u32.to_le_bytes()); // vn_aux
                out.extend_from_slice(&0u32.to_le_bytes()); // vn_next
                for (position, (_, name)) in versions.iter().enumerate() {
                    out.extend_from_slice(&0u32.to_le_bytes()); // vna_hash
                    out.extend_from_slice(&0u16.to_le_bytes()); // vna_flags
                    out.extend_from_slice(&(2 + position as u16).to_le_bytes());
                    out.extend_from_slice(&(*name as u32).to_le_bytes());
                    let next = if position + 1 == versions.len() {
                        0
                    } else {
                        16u32
                    };
                    out.extend_from_slice(&next.to_le_bytes());
                }
            }
        }
        out.resize(dynamic_offset, 0);
        for (tag, value) in dyns {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        if has_section_headers && !self.missing_section_headers {
            out.extend_from_slice(&shstrtab);
            out.resize(shdr_offset, 0);
            // (type, offset, size, link, info, entsize) per section, in order.
            let mut headers: Vec<(u32, usize, usize, u32, u32, u64)> = vec![(0, 0, 0, 0, 0, 0)];
            if has_symbols {
                headers.push((11, dynsym_offset, nsyms * SYM, 2, 1, SYM as u64)); // SHT_DYNSYM
                headers.push((3, strtab_offset, strings.len(), 0, 0, 0)); // SHT_STRTAB
                headers.push((0x6fff_ffff, versym_offset, 2 * nsyms, 1, 0, 2)); // SHT_GNU_versym
                headers.push((
                    0x6fff_fffe, // SHT_GNU_verneed
                    verneed_offset,
                    verneed_size,
                    2,
                    versions.len() as u32,
                    0,
                ));
            }
            for _ in &self.sections {
                headers.push((1, 0, 0, 0, 0, 0)); // SHT_PROGBITS, empty
            }
            headers.push((3, shstr_offset, shstrtab.len(), 0, 0, 0)); // SHT_STRTAB
            for (name, (sh_type, offset, size, link, info, entsize)) in
                name_offsets.iter().zip(headers)
            {
                out.extend_from_slice(&name.to_le_bytes()); // sh_name
                out.extend_from_slice(&sh_type.to_le_bytes());
                out.extend_from_slice(&2u64.to_le_bytes()); // SHF_ALLOC
                out.extend_from_slice(&(offset as u64).to_le_bytes()); // sh_addr
                out.extend_from_slice(&(offset as u64).to_le_bytes()); // sh_offset
                out.extend_from_slice(&(size as u64).to_le_bytes());
                out.extend_from_slice(&link.to_le_bytes());
                out.extend_from_slice(&info.to_le_bytes());
                out.extend_from_slice(&8u64.to_le_bytes()); // sh_addralign
                out.extend_from_slice(&entsize.to_le_bytes());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::file_type::FileType;

    #[test]
    fn not_elf_has_no_facts() {
        assert_eq!(ElfFacts::from_bytes(b""), None);
        assert_eq!(ElfFacts::from_bytes(b"hello"), None);
        assert_eq!(ElfFacts::from_bytes(b"MZ\0\0"), None);
        assert_eq!(ElfFacts::from_bytes(b"\x7fELF"), None);
    }

    #[test]
    fn static_executable_has_no_dynamic_facts() {
        let facts = ElfFacts::from_bytes(&TestElf::executable().build()).expect("ELF");
        assert_eq!(facts.object_type(), ElfObjectType::Executable);
        assert_eq!(facts.interpreter(), None);
        assert!(facts.needed().is_empty());
        assert!(facts.soname().is_empty());
        assert!(facts.rpath().is_empty());
        assert!(facts.runpath().is_empty());
    }

    #[test]
    fn object_type_follows_e_type_and_pie_flag() {
        for (elf, expected) in [
            (TestElf::executable(), ElfObjectType::Executable),
            (TestElf::pie_executable(), ElfObjectType::Executable),
            (TestElf::shared_object(), ElfObjectType::SharedObject),
            (
                TestElf::shared_object().needed("libc.so.6"),
                ElfObjectType::SharedObject,
            ),
            (TestElf::relocatable(), ElfObjectType::Relocatable),
            (TestElf::new(header::ET_CORE), ElfObjectType::Other),
        ] {
            let facts = ElfFacts::from_bytes(&elf.build()).expect("ELF");
            assert_eq!(facts.object_type(), expected, "{elf:?}");
        }
    }

    #[test]
    fn section_names_follow_the_table_with_the_null_section_first() {
        let facts = ElfFacts::from_bytes(
            &TestElf::shared_object()
                .import("puts", Some("GLIBC_2.2.5"))
                .section(".note")
                .section(".comment")
                .build(),
        )
        .expect("ELF");
        assert_eq!(
            facts.section_names(),
            [
                "",
                ".dynsym",
                ".dynstr",
                ".gnu.version",
                ".gnu.version_r",
                ".note",
                ".comment",
                ".shstrtab",
            ]
        );
    }

    #[test]
    fn section_names_are_empty_without_a_section_table() {
        let facts = ElfFacts::from_bytes(&TestElf::executable().build()).expect("ELF");
        assert!(facts.section_names().is_empty());
        assert!(facts.errors().is_empty());
    }

    #[test]
    fn section_table_past_end_of_file_is_a_readelf_error() {
        let facts = ElfFacts::from_bytes(
            &TestElf::shared_object()
                .import("puts", None)
                .missing_section_headers()
                .build(),
        )
        .expect("ELF");
        assert!(facts.section_names().is_empty());
        assert_eq!(
            facts.errors(),
            [
                "In ELF header: Reading 512 bytes extends past end of file for section headers"
                    .to_string(),
                "In ELF header: Section headers are not available!".to_string(),
            ]
        );
    }

    #[test]
    fn interpreter_outside_the_file_is_a_readelf_error() {
        let mut elf = TestElf::executable()
            .interpreter("/lib64/ld-linux-x86-64.so.2")
            .build();
        // PT_INTERP is the second program header (offset 64 + 56); set its
        // p_filesz past the end of the file, as a stripped debug file with
        // no contents at that offset has it.
        elf[64 + 56 + 32..64 + 56 + 40].copy_from_slice(&0x1000u64.to_le_bytes());
        let facts = ElfFacts::from_bytes(&elf).expect("ELF");
        assert_eq!(
            facts.errors(),
            ["In program headers: Unable to find program interpreter name".to_string()]
        );
    }

    #[test]
    fn empty_interpreter_segment_is_a_readelf_error() {
        let mut elf = TestElf::executable()
            .interpreter("/lib64/ld-linux-x86-64.so.2")
            .build();
        // PT_INTERP's p_filesz is zero, as in `objcopy --only-keep-debug`
        // output; its p_memsz is left alone.
        elf[64 + 56 + 32..64 + 56 + 40].copy_from_slice(&0u64.to_le_bytes());
        let facts = ElfFacts::from_bytes(&elf).expect("ELF");
        assert_eq!(
            facts.errors(),
            ["In program headers: Unable to find program interpreter name".to_string()]
        );
    }

    #[test]
    fn class_endianness_and_machine_come_from_the_header() {
        let facts = ElfFacts::from_bytes(&header_only(ElfClass::Elf32, ElfEndianness::Big, 8))
            .expect("ELF");
        assert_eq!(facts.class(), ElfClass::Elf32);
        assert_eq!(facts.endianness(), ElfEndianness::Big);
        assert_eq!(facts.machine(), 8);

        let x86 = ElfFacts::from_bytes(&TestElf::executable().build()).expect("ELF");
        assert_eq!(x86.class(), ElfClass::Elf64);
        assert_eq!(x86.endianness(), ElfEndianness::Little);
        assert_eq!(x86.machine(), 62);
    }

    #[test]
    fn dynamic_facts_in_order() {
        let bytes = TestElf::pie_executable()
            .interpreter("/lib64/ld-linux-x86-64.so.2")
            .needed("libfoo.so.1")
            .needed("libc.so.6")
            .soname("libbar.so.2")
            .rpath("/usr/lib/foo:$ORIGIN/../lib")
            .runpath("/opt/lib")
            .build();
        let facts = ElfFacts::from_bytes(&bytes).expect("ELF");
        assert_eq!(facts.object_type(), ElfObjectType::Executable);
        assert_eq!(facts.interpreter(), Some("/lib64/ld-linux-x86-64.so.2"));
        assert_eq!(facts.needed(), ["libfoo.so.1", "libc.so.6"]);
        assert_eq!(facts.soname(), ["libbar.so.2"]);
        assert_eq!(facts.rpath(), ["/usr/lib/foo", "$ORIGIN/../lib"]);
        assert_eq!(facts.runpath(), ["/opt/lib"]);
    }

    #[test]
    fn hardening_facts() {
        let plain =
            ElfFacts::from_bytes(&TestElf::executable().needed("libc.so.6").build()).expect("ELF");
        assert!(plain.is_fixed_address_executable());
        assert!(!plain.has_relro());
        assert_eq!(plain.stack_permissions(), None);
        assert!(!plain.has_textrel());
        assert!(!plain.binds_now());

        let hardened = ElfFacts::from_bytes(
            &TestElf::pie_executable()
                .needed("libc.so.6")
                .relro()
                .stack(6)
                .bind_now()
                .build(),
        )
        .expect("ELF");
        assert!(!hardened.is_fixed_address_executable());
        assert!(hardened.has_relro());
        assert_eq!(hardened.stack_permissions().as_deref(), Some("rw-"));
        assert!(hardened.binds_now());

        let executable_stack = ElfFacts::from_bytes(
            &TestElf::shared_object()
                .soname("libx.so.1")
                .stack(7)
                .textrel()
                .build(),
        )
        .expect("ELF");
        assert_eq!(executable_stack.stack_permissions().as_deref(), Some("rwx"));
        assert!(executable_stack.has_textrel());
    }

    #[test]
    fn dynamic_segment_without_content_is_a_bad_dynamic_table() {
        let good = ElfFacts::from_bytes(&TestElf::shared_object().needed("libc.so.6").build())
            .expect("ELF");
        assert!(!good.has_bad_dynamic_table());
        let none = ElfFacts::from_bytes(&TestElf::executable().build()).expect("ELF");
        assert!(!none.has_bad_dynamic_table());
        let split = ElfFacts::from_bytes(&TestElf::shared_object().no_dynamic_content().build())
            .expect("ELF");
        assert!(split.has_bad_dynamic_table());
        assert!(split.needed().is_empty());
    }

    #[test]
    fn bind_now_from_the_dt_flags_1_now_flag() {
        let bytes = TestElf::shared_object().needed("libc.so.6").build();
        assert!(!ElfFacts::from_bytes(&bytes).expect("ELF").binds_now());
        // `-z now -pie` writes `DT_FLAGS_1` with `DF_1_NOW | DF_1_PIE`.
        let bytes = TestElf::pie_executable().flags_1_now().build();
        let facts = ElfFacts::from_bytes(&bytes).expect("ELF");
        assert!(facts.binds_now());
        assert!(facts.is_pie());
    }

    /// `file(1)` and `readelf` still name an ELF whose Section header table
    /// is cut off; File type (a full goblin parse) does not.
    #[test]
    fn missing_section_headers_keep_facts() {
        let bytes = TestElf::executable()
            .interpreter("/lib/klibc-abc.so")
            .missing_section_headers()
            .build();
        assert_eq!(FileType::from_bytes(&bytes), None);
        let facts = ElfFacts::from_bytes(&bytes).expect("ELF");
        assert_eq!(facts.object_type(), ElfObjectType::Executable);
        assert_eq!(facts.interpreter(), Some("/lib/klibc-abc.so"));
    }

    #[test]
    fn cut_off_program_headers_leave_dynamic_facts_empty() {
        let bytes = TestElf::shared_object().needed("libc.so.6").build();
        let facts = ElfFacts::from_bytes(&bytes[..80]).expect("ELF header is whole");
        assert_eq!(facts.object_type(), ElfObjectType::SharedObject);
        assert!(facts.needed().is_empty());
    }

    #[test]
    fn host_binaries() -> anyhow::Result<()> {
        let bytes = std::fs::read(std::env::current_exe()?)?;
        let facts = ElfFacts::from_bytes(&bytes).expect("test binary is ELF");
        assert_eq!(facts.object_type(), ElfObjectType::Executable);
        assert!(facts.interpreter().is_some(), "{facts:?}");
        assert!(
            facts
                .needed()
                .iter()
                .any(|name| name.starts_with("libc.so")),
            "{facts:?}"
        );
        for path in [
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/usr/lib/x86_64-linux-gnu/libc.so.6",
        ] {
            let path = Path::new(path);
            if path.is_file() {
                let facts = ElfFacts::from_bytes(&std::fs::read(path)?).expect("libc is ELF");
                assert_eq!(facts.soname(), ["libc.so.6"], "{}", path.display());
            }
        }
        Ok(())
    }

    /// The test binary imports libc functions, each with the version it needs.
    #[test]
    fn host_binary_dynamic_symbols_carry_versions() -> anyhow::Result<()> {
        let bytes = std::fs::read(std::env::current_exe()?)?;
        let facts = ElfFacts::from_bytes(&bytes).expect("test binary is ELF");
        let imported: Vec<_> = facts
            .symbols()
            .iter()
            .filter(|symbol| symbol.is_undefined() && symbol.version().starts_with("GLIBC_"))
            .collect();
        assert!(!imported.is_empty(), "{:?}", facts.symbols());
        assert!(
            facts
                .symbols()
                .iter()
                .all(|symbol| !symbol.name().is_empty() && !symbol.name().contains('@')),
            "{:?}",
            facts.symbols()
        );
        Ok(())
    }

    #[test]
    fn imported_symbols_carry_the_version_they_need() {
        let bytes = TestElf::executable()
            .needed("libc.so.6")
            .import("printf", Some("GLIBC_2.2.5"))
            .import("__fentry__", Some("GLIBC_2.13"))
            .import("weak_thing", None)
            .import("puts", Some("GLIBC_2.2.5"))
            .build();
        let facts = ElfFacts::from_bytes(&bytes).expect("ELF");
        let symbols: Vec<_> = facts
            .symbols()
            .iter()
            .map(|symbol| (symbol.name(), symbol.is_undefined(), symbol.version()))
            .collect();
        assert_eq!(
            symbols,
            [
                ("printf", true, "GLIBC_2.2.5"),
                ("__fentry__", true, "GLIBC_2.13"),
                ("weak_thing", true, ""),
                ("puts", true, "GLIBC_2.2.5"),
            ]
        );
        assert_eq!(facts.needed(), ["libc.so.6"]);
    }

    #[test]
    fn symbols_are_empty_without_a_dynamic_symbol_table() {
        let facts =
            ElfFacts::from_bytes(&TestElf::executable().needed("libc.so.6").build()).expect("ELF");
        assert!(facts.symbols().is_empty());
    }

    #[test]
    fn elf_files_index_regular_files_and_hardlinks() {
        let elf = TestElf::executable().build();
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("usr/bin/static", None, Some(elf.clone())),
                Entry::hardlink("usr/bin/link", None, Some(elf)),
                Entry::regular_file("usr/share/doc/README", None, Some(b"text".to_vec())),
                Entry::symlink("usr/bin/alias", "static"),
            ],
        );
        let files = ElfFiles::from_tree(&tree);
        let paths: Vec<_> = files.iter().map(|(path, _)| path.to_path_buf()).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("usr/bin/link"),
                PathBuf::from("usr/bin/static")
            ]
        );
        assert!(files.get("usr/share/doc/README").is_none());
    }
}
