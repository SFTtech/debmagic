//! Duplicate soname lines in the shlibs control file.
//!
//! A line's identity is the optional `udeb:` prefix, the library name, and
//! the version token. The dependency that follows is not part of the key.
//! This is the file text, not an ELF SONAME.

use std::collections::HashMap;

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
