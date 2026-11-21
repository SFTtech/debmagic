//! Duplicate soname lines in the shlibs control file.
//!
//! A line's identity is the optional `udeb:` prefix, the library name, and
//! the version token. The dependency that follows is not part of the key.
//! This is the file text, not an ELF SONAME.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Location;
use crate::lint::rules::lintian::fields_relations_more::architectures::is_ldconfig_folder;

use super::lines;

pub(super) fn at() -> Location {
    Location {
        path: "shlibs".into(),
        line: None,
        column: None,
    }
}

pub(super) fn inspect(ctx: &mut BinaryPackageContext<'_>) -> Option<Vec<String>> {
    let text = lines::read_text(ctx, "shlibs")?;
    Some(parse(&text))
}

pub(super) fn parse(text: &str) -> Vec<String> {
    let mut positions: HashMap<String, Vec<u32>> = HashMap::new();
    for (index, line) in lines::perl_lines(text).into_iter().enumerate() {
        let position = index as u32 + 1;
        if lines::is_blank_or_comment(line) {
            continue;
        }
        let (udeb, body) = strip_udeb(line);
        let mut fields = body.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let version = fields.next().unwrap_or("");
        let pretty = format!("{udeb}{name} {version}");
        positions.entry(pretty).or_default().push(position);
    }
    lines::duplicate_groups(&positions, str::to_string)
}

fn strip_udeb(line: &str) -> (&str, &str) {
    let Some(rest) = line.strip_prefix("udeb:") else {
        return ("", line);
    };
    if rest.starts_with(char::is_whitespace) {
        ("udeb: ", rest.trim_start())
    } else {
        ("", line)
    }
}

/// The `shlibs` control file as lintian's `lookup('shlibs')` sees it: `None`
/// when it is absent or not a file (a symlink does not count). Otherwise its
/// text, read as empty when it is not UTF-8 (`decoded_utf8`).
pub(super) fn control_text(ctx: &mut BinaryPackageContext<'_>) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get("shlibs")?;
    if !entry.is_file() {
        return None;
    }
    let bytes = files.read(entry).unwrap_or_default();
    Some(std::str::from_utf8(&bytes).unwrap_or_default().to_string())
}

/// Data files that are shared libraries with a versioned SONAME: lintian's
/// `soname_by_filename` minus its unversioned ones. Path order.
pub(super) fn versioned_libraries(ctx: &mut BinaryPackageContext<'_>) -> Vec<PathBuf> {
    let Some(elf) = ctx.data_elf_files() else {
        return Vec::new();
    };
    elf.iter()
        .filter(|(_, facts)| {
            facts
                .soname()
                .first()
                .is_some_and(|soname| has_version(soname))
        })
        .map(|(path, _)| path.to_path_buf())
        .collect()
}

/// Whether lintian's `human_soname` of a SONAME has a space: a `.so.`
/// infix, or a `-<digit>` before a `.so` suffix.
fn has_version(soname: &str) -> bool {
    if soname.contains(".so.") {
        return true;
    }
    soname.strip_suffix(".so").is_some_and(|stem| {
        stem.match_indices('-')
            .any(|(index, _)| stem[index + 1..].starts_with(|c: char| c.is_ascii_digit()))
    })
}

/// Whether the directory of `path` is one of lintian's `ldconfig_folders`.
pub(super) fn in_ldconfig_folder(path: &Path) -> bool {
    path.parent()
        .and_then(Path::to_str)
        .is_some_and(is_ldconfig_folder)
}

static NSS_PLUGIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*/)?libnss_[^.]+\.so\.\d+$").expect("nss plugin pattern"));

/// lintian's `is_nss_plugin`: a libc NSS module, which has no shlibs entry.
pub(super) fn is_nss_plugin(path: &Path) -> bool {
    path.to_str().is_some_and(|path| NSS_PLUGIN.is_match(path))
}

/// The prerequisites of the non-udeb lines of the shlibs file, each line's
/// dependencies joined by single spaces, once each, in file order. A line
/// with no dependencies gives the empty prerequisite.
pub(super) fn prerequisites(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut prerequisites = Vec::new();
    for line in lines::perl_lines(text) {
        if lines::is_blank_or_comment(line) {
            continue;
        }
        let (udeb, body) = strip_udeb(line);
        if !udeb.is_empty() {
            continue;
        }
        let prerequisite = body
            .split_whitespace()
            .skip(2)
            .collect::<Vec<_>>()
            .join(" ");
        if seen.insert(prerequisite.clone()) {
            prerequisites.push(prerequisite);
        }
    }
    prerequisites
}

/// lintian's `m/\(\s*[><](?![<>=])\s*/`: an obsolete `<` or `>` operator in
/// a versioned dependency. `>>`, `<<`, `>=`, and `<=` are not.
pub(super) fn has_obsolete_operator(prerequisite: &str) -> bool {
    prerequisite.match_indices('(').any(|(index, _)| {
        let mut rest = prerequisite[index + 1..].trim_start().chars();
        match rest.next() {
            Some('<' | '>') => !matches!(rest.next(), Some('<' | '>' | '=')),
            _ => false,
        }
    })
}

#[cfg(test)]
pub(super) const LIBBAZ_SHLIBS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/lintian-parity/checks/debian/shlibs/legacy-libbaz/debian/lib.shlibs"
));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_libbaz_matches_the_recipe() {
        assert_eq!(
            parse(LIBBAZ_SHLIBS),
            vec![
                "(lines 1 2) libdoesntexist2 1.0".to_string(),
                "(lines 7 8) udeb: libdoesntexist2 1.0".to_string(),
            ]
        );
    }

    #[test]
    fn comments_and_distinct_sonames_are_quiet() {
        let text = "\
# comment

libfoo 1 pkg
udeb: libfoo 1 other
";
        assert!(parse(text).is_empty());
    }
}
