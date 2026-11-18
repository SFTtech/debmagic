//! The `conffiles` member, shared by the `conffiles` Check's Rules.

use crate::lint::context::BinaryPackageContext;

/// A `conffiles` line: the path with one leading `/` removed, and its
/// 1-based line number.
pub(super) struct Declared {
    pub(super) path: String,
    pub(super) line: u32,
}

/// The `conffiles` entries in file order. Empty when the member is missing
/// or not UTF-8.
pub(super) fn declared(ctx: &mut BinaryPackageContext<'_>) -> Vec<Declared> {
    let Some(files) = ctx.control_files() else {
        return Vec::new();
    };
    let Some(entry) = files.get("conffiles") else {
        return Vec::new();
    };
    let Ok(bytes) = files.read(entry) else {
        return Vec::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Vec::new();
    };
    parse(&text)
}

fn parse(text: &str) -> Vec<Declared> {
    text.split('\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let path = line.split_whitespace().next_back()?;
            let path = path.strip_prefix('/').unwrap_or(path);
            (!path.is_empty()).then(|| Declared {
                path: path.to_string(),
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keeps_line_numbers_and_strips_one_slash() {
        let lines: Vec<_> = parse("/etc/foo\n\nremove-on-upgrade //etc/bar  \n  \n etc/baz")
            .into_iter()
            .map(|entry| (entry.line, entry.path))
            .collect();
        assert_eq!(
            lines,
            vec![
                (1, "etc/foo".to_string()),
                (3, "/etc/bar".to_string()),
                (5, "etc/baz".to_string()),
            ]
        );
    }
}
