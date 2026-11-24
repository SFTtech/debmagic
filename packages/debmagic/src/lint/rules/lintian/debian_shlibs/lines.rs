//! Shared control-file text helpers for the `debian/shlibs` Check.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::elf::ElfFiles;
use crate::lint::file_tree::EntryKind;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch_triplets;
use crate::lint::text::perl_lines;

static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("blank line"));

pub(super) fn is_blank_or_comment(line: &str) -> bool {
    BLANK.is_match(line) || line.starts_with('#')
}

pub(super) fn read_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let bytes = files.read_file(name)?;
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
static NSS_PLUGIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*/)?libnss_[^.]+\.so\.\d+$").expect("nss plugin"));
static SONAME_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname line")
});

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

/// `is_nss_plugin` of the Check, on the full path.
pub(super) fn is_nss_plugin(path: &Path) -> bool {
    NSS_PLUGIN.is_match(&path.to_string_lossy())
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

/// `name version` and `udeb: name version` keys of the shlibs text, as the
/// Check's `shlibs_positions_by_pretty_soname` keys them.
pub(super) fn shlibs_pretty_sonames(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for line in perl_lines(text) {
        if is_blank_or_comment(line) {
            continue;
        }
        let (udeb, body) = match body_after_udeb(line) {
            Some(body) => ("udeb: ", body),
            None => ("", line),
        };
        let mut fields = body.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let version = fields.next().unwrap_or("");
        keys.insert(format!("{udeb}{name} {version}"));
    }
    keys
}

fn body_after_udeb(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("udeb:")?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// Sonames named by the symbols text (`symbols_positions_by_soname` keys),
/// as written in the file.
pub(super) fn symbols_sonames(text: &str) -> HashSet<String> {
    perl_lines(text)
        .into_iter()
        .filter(|line| !is_blank_or_comment(line))
        .filter_map(|line| SONAME_LINE.captures(line))
        .map(|captures| captures[1].to_string())
        .collect()
}

/// Text of the symbols control file, if one exists. A file that is not a
/// regular file or hardlink reads as empty, and invalid UTF-8 reads as empty,
/// as lintian's `decoded_utf8` does.
pub(super) fn symbols_text(ctx: &mut BinaryPackageContext<'_>) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get("symbols")?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return Some(String::new());
    }
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8(bytes).unwrap_or_default())
}
