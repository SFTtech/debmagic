//! Shared control-file text helpers for the `debian/shlibs` Check.

use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::elf::ElfFiles;
use crate::lint::file_tree::EntryKind;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch_triplets;

static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("blank line"));

/// Perl `split(/\n/, ...)` : trailing empty fields are dropped, blank lines in
/// the middle stay, and a final newline does not invent an extra line.
pub(super) fn perl_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

pub(super) fn is_blank_or_comment(line: &str) -> bool {
    BLANK.is_match(line) || line.starts_with('#')
}

/// Perl `split` with a pattern: keep a leading or middle empty field, drop
/// trailing empty fields.
pub(super) fn perl_split<'a>(pattern: &Regex, text: &'a str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = pattern.split(text).collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

pub(super) fn read_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get(name)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `(lines 1 2) pretty`, with line numbers sorted ascending.
pub(super) fn duplicate_groups(
    positions: &HashMap<String, Vec<u32>>,
    pretty: impl Fn(&str) -> String,
) -> Vec<String> {
    let mut groups = Vec::new();
    for (name, lines) in positions {
        if lines.len() < 2 {
            continue;
        }
        let first = lines.iter().copied().min().unwrap_or(0);
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        let mut message = String::from("(lines");
        for line in sorted {
            message.push(' ');
            message.push_str(&line.to_string());
        }
        message.push_str(") ");
        message.push_str(&pretty(name));
        groups.push((first, message));
    }
    groups.sort_by_key(|(first, _)| *first);
    groups.into_iter().map(|(_, message)| message).collect()
}

// Shared-library helpers for LN1036 to LN1039 (`debian/shlibs` Check).

static SO_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)\.so\.(.*)$").expect("lib.so.version"));
static DASH_SO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)-(\d.*)\.so$").expect("lib-version.so"));

/// `human_soname` of the Check: `name version`, or the SONAME unchanged.
pub(super) fn human_soname(soname: &str) -> String {
    if let Some(captures) = SO_VERSION.captures(soname) {
        return format!("{} {}", &captures[1], &captures[2]);
    }
    if let Some(captures) = DASH_SO.captures(soname) {
        return format!("{} {}", &captures[1], &captures[2]);
    }
    soname.to_string()
}
/// `dirname($path) . "/"` is one of lintian's `ldconfig_folders`: the
/// multiarch `lib/<triplet>` and `usr/lib/<triplet>`, plus the fixed list.
pub(super) fn in_ldconfig_folder(path: &Path) -> bool {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return false;
    };
    let mut folder = parent.as_os_str().as_bytes().to_vec();
    folder.push(b'/');
    ldconfig_folders()
        .iter()
        .any(|candidate| candidate.as_bytes() == folder.as_slice())
}

fn ldconfig_folders() -> Vec<String> {
    let mut folders: Vec<String> = Vec::new();
    for triplet in deb_host_multiarch_triplets() {
        folders.push(format!("lib/{triplet}/"));
        folders.push(format!("usr/lib/{triplet}/"));
    }
    for dir in [
        "lib",
        "lib32",
        "lib64",
        "libx32",
        "usr/lib",
        "usr/lib32",
        "usr/lib64",
        "usr/libx32",
        "usr/local/lib",
    ] {
        folders.push(format!("{dir}/"));
    }
    folders
}

/// ELF SONAMEs of the Data files, in path order: `(path, SONAME)` for each
/// ELF file that has an SONAME entry.
pub(super) fn elf_sonames(elf: &ElfFiles) -> Vec<(PathBuf, String)> {
    elf.iter()
        .filter_map(|(path, facts)| {
            let soname = facts.soname().first()?;
            Some((path.to_path_buf(), soname.clone()))
        })
        .collect()
}
