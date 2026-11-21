//! Duplicate soname lines in the shlibs control file.
//!
//! A line's identity is the optional `udeb:` prefix, the library name, and
//! the version token. The dependency that follows is not part of the key.
//! This is the file text, not an ELF SONAME.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Location;

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
