//! The `Description` field as lintian's `fields/description` Check splits it.

use crate::lint::context::BinaryPackageContext;

pub(super) struct Description {
    pub(super) synopsis: String,
    /// Lines of the extended description, as lintian numbers them (position
    /// 1 is the first). Perl `split /\n/` drops trailing empty lines.
    pub(super) lines: Vec<String>,
}

impl Description {
    pub(super) fn parse(value: &str) -> Self {
        match value.split_once('\n') {
            Some((synopsis, extended)) => {
                let mut lines: Vec<String> = extended.split('\n').map(str::to_string).collect();
                while lines.last().is_some_and(|line| line.is_empty()) {
                    lines.pop();
                }
                Self {
                    synopsis: synopsis.to_string(),
                    lines,
                }
            }
            None => Self {
                synopsis: value.trim().to_string(),
                lines: Vec::new(),
            },
        }
    }

    /// Extended lines with their 1-based position, without the ` .` blank
    /// separators (which still count towards the positions).
    pub(super) fn numbered_lines(&self) -> impl Iterator<Item = (usize, &str)> {
        self.lines
            .iter()
            .enumerate()
            .map(|(index, line)| (index + 1, line.as_str()))
            .filter(|(_, line)| !is_blank_separator(line))
    }
}

/// `^ \.\s*$`
fn is_blank_separator(line: &str) -> bool {
    line.strip_prefix(" .")
        .is_some_and(|rest| rest.chars().all(char::is_whitespace))
}

/// The Description of the Binary package's Installation control, if declared.
pub(super) fn description(ctx: &BinaryPackageContext<'_>) -> Option<Description> {
    ctx.installation_control()
        .fields()
        .untrimmed_value("Description")
        .map(Description::parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_synopsis_and_extended_lines() {
        let description = Description::parse("syn\n one\n .\n two\n");
        assert_eq!(description.synopsis, "syn");
        let lines: Vec<_> = description.numbered_lines().collect();
        assert_eq!(lines, [(1, " one"), (3, " two")]);
        assert!(Description::parse("  syn  ").lines.is_empty());
        assert_eq!(Description::parse("  syn  ").synopsis, "syn");
    }
}
