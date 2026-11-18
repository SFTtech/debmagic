//! The `conffiles` member, shared by the `conffiles` Check's Rules.

use crate::lint::context::BinaryPackageContext;

/// The `conffiles` paths in file order, with one leading `/` removed. Empty
/// when the member is missing or not UTF-8.
pub(super) fn declared(ctx: &mut BinaryPackageContext<'_>) -> Vec<String> {
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

fn parse(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let path = line.split_whitespace().next_back()?;
            let path = path.strip_prefix('/').unwrap_or(path);
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_takes_last_word_and_strips_one_slash() {
        assert_eq!(
            parse("/etc/foo\n\nremove-on-upgrade //etc/bar  \n  \n etc/baz"),
            vec!["etc/foo", "/etc/bar", "etc/baz"]
        );
    }
}
