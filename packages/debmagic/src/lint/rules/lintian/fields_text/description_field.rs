//! The `Description` field as lintian's `fields/description` Check splits it.
//!
//! Same split as `description/description_field`: synopsis is the first line
//! (trimmed only when the field has no newline); extended lines keep their
//! indentation; Perl `split /\n/` drops trailing empty lines.

use crate::lint::context::BinaryPackageContext;

pub(super) struct Description {
    pub(super) synopsis: String,
    /// Lines of the extended description. Position 1 is the first.
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
pub(super) fn is_blank_separator(line: &str) -> bool {
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
