//! Debconf templates as lintian's Deb822 parser reads them with
//! `DCTRL_DEBCONF_TEMPLATE`.
//!
//! A `#` line is a comment. Only a completely empty line ends a paragraph;
//! a whitespace-only line is a syntax error, and so is a duplicate field or
//! a continuation outside a paragraph. Any error drops the whole file, which
//! is what the Check does when `read_file` dies. Field lookup is
//! case-insensitive. `value` trims both ends. Known field names are
//! case-sensitive and ignore one leading `_` and a language suffix.

use std::sync::LazyLock;

use regex::Regex;

use super::files::{self, TextFile};
use crate::lint::file_tree::FileTree;

/// Valid template fields from debconf-devel(7), as the Check lists them,
/// including `Indices` and `Help`.
const TEMPLATE_FIELDS: &[&str] = &[
    "Template",
    "Type",
    "Choices",
    "Indices",
    "Default",
    "Description",
    "Help",
];

static FIELD_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^: \t]+):\s*(.*)$").expect("valid regex"));
static CHOICE_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\\[, ]|,\s+)").expect("valid regex"));
static ESCAPED_CHOICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\\([, ])$").expect("valid regex"));
static CHOICE_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^,\s+$").expect("valid regex"));
static INTERNAL_USE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)for internal use").expect("valid regex"));

pub(super) struct TemplatesFile {
    pub path: std::path::PathBuf,
    pub templates: Vec<Template>,
}

pub(super) struct Template {
    fields: Vec<Field>,
}

pub(super) struct Field {
    pub name: String,
    pub line: u32,
    verbatim: String,
}

impl Template {
    pub(super) fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub(super) fn declares(&self, name: &str) -> bool {
        self.field(name).is_some()
    }

    /// Trimmed value. Empty when the field is absent.
    pub(super) fn value(&self, name: &str) -> String {
        self.field(name)
            .map(|field| field.verbatim.trim().to_string())
            .unwrap_or_default()
    }

    pub(super) fn position(&self, name: &str) -> Option<u32> {
        self.field(name).map(|field| field.line)
    }

    /// Line of the first field: lintian's paragraph position.
    pub(super) fn paragraph_line(&self) -> Option<u32> {
        self.fields.first().map(|field| field.line)
    }

    fn field(&self, name: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }
}

/// `Template` value, or `no-template-name` when that field is absent.
pub(super) fn template_name(template: &Template) -> String {
    if template.declares("Template") {
        template.value("Template")
    } else {
        "no-template-name".to_string()
    }
}

/// First line of `Description`, or empty when that field is missing or blank.
pub(super) fn short_description(template: &Template) -> String {
    let description = template.value("Description");
    if description.is_empty() {
        return String::new();
    }
    description
        .split_once('\n')
        .map(|(short, _)| short.to_string())
        .unwrap_or(description)
}

pub(super) fn for_internal_use(short: &str) -> bool {
    INTERNAL_USE.is_match(short)
}

/// Developer's Reference limit for a template synopsis, in characters.
pub(super) const MAXIMUM_SHORT_LENGTH: usize = 75;

/// Developer's Reference limit for an extended description, in wrapped lines.
pub(super) const MAXIMUM_EXTENDED_LINES: usize = 20;

/// Text after the first newline of `Description`.
///
/// `None` when the field has no newline, which is how the Check leaves
/// `$extended` undefined. The string may be empty.
pub(super) fn extended_description(template: &Template) -> Option<String> {
    template
        .value("Description")
        .split_once('\n')
        .map(|(_, rest)| rest.to_string())
}

/// Lines `$extended` occupies after the Check wraps at 80 characters.
///
/// A line longer than 80 breaks at the last space at or before column 80.
/// With no such space, it breaks at the first space anywhere. With no space,
/// the whole line counts as one.
pub(super) fn wrapped_line_count(extended: &str) -> usize {
    const MAXIMUM_LINE_LENGTH: usize = 80;
    let mut lines = 0usize;
    for piece in extended.split('\n') {
        let mut rest: Vec<char> = piece.chars().collect();
        while rest.len() > MAXIMUM_LINE_LENGTH {
            let within = rest
                .iter()
                .take(MAXIMUM_LINE_LENGTH + 1)
                .rposition(|ch| *ch == ' ');
            let index = within.or_else(|| rest.iter().position(|ch| *ch == ' '));
            match index {
                None => rest.clear(),
                Some(index) => {
                    rest = rest.split_off(index + 1);
                    lines += 1;
                }
            }
        }
        lines += 1;
    }
    lines
}

/// Parsed templates when the installable Check would reach them.
pub(super) fn load_gated(files: &FileTree) -> Option<TemplatesFile> {
    if !files::gate(files).open() {
        return None;
    }
    load_templates(files)
}

/// `Choices` and `Choices-ll`, plus a single leading `_`, are known.
/// `__Choices` is not: only one underscore is removed.
pub(super) fn known_field(name: &str) -> bool {
    let main = name.split_once('-').map(|(head, _)| head).unwrap_or(name);
    let stripped = main.strip_prefix('_').unwrap_or(main);
    TEMPLATE_FIELDS.contains(&stripped)
}

/// Parsed `templates`, or `None` when the member is missing, not UTF-8, or
/// not valid debconf Deb822.
pub(super) fn load_templates(files: &FileTree) -> Option<TemplatesFile> {
    let TextFile { path, text } = files::read_utf8(files, "templates")?;
    let templates = parse(&text)?;
    Some(TemplatesFile { path, templates })
}

pub(super) fn parse(text: &str) -> Option<Vec<Template>> {
    let mut templates = Vec::new();
    let mut current: Option<Template> = None;
    for (index, line) in physical_lines(text).into_iter().enumerate() {
        let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            if let Some(template) = current.take() {
                templates.push(template);
            }
            continue;
        }
        if line.chars().all(char::is_whitespace) {
            return None;
        }
        if let Some((name, value)) = field_line(line) {
            let template = current.get_or_insert_with(|| Template { fields: Vec::new() });
            if template.fields.iter().any(|field| field.name == name) {
                return None;
            }
            template.fields.push(Field {
                name,
                line: line_no,
                verbatim: value,
            });
            continue;
        }
        if is_continuation(line) {
            let template = current.as_mut()?;
            let field = template.fields.last_mut()?;
            field.verbatim.push('\n');
            field.verbatim.push_str(line.trim_end());
            continue;
        }
        return None;
    }
    if let Some(template) = current {
        templates.push(template);
    }
    Some(templates)
}

/// Choice count from debconf 1.5.8, as the Check copies it.
///
/// `,\s+` separates items. `\,` and `\ ` stay inside an item.
pub(super) fn count_choices(choices: &str) -> usize {
    let mut items = Vec::new();
    let mut item = String::new();
    for chunk in perl_split(&CHOICE_SPLIT, choices) {
        if let Some(captures) = ESCAPED_CHOICE.captures(&chunk) {
            item.push_str(&captures[1]);
        } else if CHOICE_SEPARATOR.is_match(&chunk) {
            items.push(std::mem::take(&mut item));
        } else {
            item.push_str(&chunk);
        }
    }
    if !item.is_empty() {
        items.push(item);
    }
    items.len()
}

fn field_line(line: &str) -> Option<(String, String)> {
    let captures = FIELD_LINE.captures(line)?;
    let name = captures[1].to_string();
    let value = captures[2].trim_end().to_string();
    Some((name, value))
}

fn is_continuation(line: &str) -> bool {
    let mut chars = line.chars();
    matches!(chars.next(), Some(' ' | '\t')) && line.chars().any(|ch| !ch.is_whitespace())
}

/// Perl `split` with a capturing separator: the separator is kept, a leading
/// empty field stays, and trailing empty fields go. An empty string has no
/// fields.
fn perl_split(separator: &Regex, value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut last = 0;
    for found in separator.find_iter(value) {
        parts.push(value[last..found.start()].to_string());
        parts.push(found.as_str().to_string());
        last = found.end();
    }
    parts.push(value[last..].to_string());
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

fn physical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        while lines.last() == Some(&"") {
            lines.pop();
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_choices_like_debconf() {
        assert_eq!(count_choices(""), 0);
        assert_eq!(count_choices("sakana, sake"), 2);
        assert_eq!(count_choices("x,y"), 1);
        assert_eq!(count_choices("foo\\, bar, boo"), 2);
        assert_eq!(count_choices("foo, bar, boo"), 3);
        assert_eq!(count_choices("a, b, c"), 3);
    }

    #[test]
    fn known_fields_keep_language_and_one_underscore() {
        assert!(known_field("Description"));
        assert!(known_field("Choices-de.UTF-8"));
        assert!(known_field("_Choices"));
        assert!(known_field("Indices"));
        assert!(known_field("Help"));
        assert!(!known_field("description"));
        assert!(!known_field("description-de.UTF-8"));
        assert!(!known_field("__Choices"));
        assert!(!known_field("Unknown-field"));
    }

    #[test]
    fn syntax_error_drops_the_file() {
        assert!(parse("Template: a/b\nType: string\nTemplate: a/c\n").is_none());
        assert!(parse("Template: a/b\n \nType: string\n").is_none());
        assert!(parse("Template: a/b\nDescription whoops\n").is_none());
        let parsed = parse("Template: a/b\n# comment\nWeird: 1\n").expect("comments are allowed");
        assert_eq!(parsed[0].fields()[1].line, 3);
        assert_eq!(parsed[0].value("Template"), "a/b");
    }

    #[test]
    fn wraps_extended_lines_like_the_check() {
        assert_eq!(wrapped_line_count(&"a".repeat(80)), 1);
        assert_eq!(wrapped_line_count(&"a".repeat(81)), 1);
        assert_eq!(
            wrapped_line_count(&format!("{} {}", "a".repeat(40), "b".repeat(50))),
            2
        );
        assert_eq!(wrapped_line_count(&format!("{} x", "a".repeat(81))), 2);
        assert_eq!(wrapped_line_count("a\n\nb"), 3);
        let twenty = vec!["x"; 20].join("\n");
        let twenty_one = vec!["x"; 21].join("\n");
        assert_eq!(wrapped_line_count(&twenty), 20);
        assert_eq!(wrapped_line_count(&twenty_one), 21);
        assert!(wrapped_line_count(&twenty) <= MAXIMUM_EXTENDED_LINES);
        assert!(wrapped_line_count(&twenty_one) > MAXIMUM_EXTENDED_LINES);
    }

    #[test]
    fn extended_description_is_the_text_after_the_first_newline() {
        let parsed = parse("Template: a/b\nDescription: Short\n The rest\n").expect("parsed");
        assert_eq!(
            extended_description(&parsed[0]).as_deref(),
            Some(" The rest")
        );
        let one_line = parse("Template: a/b\nDescription: Short\n").expect("parsed");
        assert_eq!(extended_description(&one_line[0]), None);
        assert_eq!(one_line[0].paragraph_line(), Some(1));
    }

    #[test]
    fn description_lookup_is_case_insensitive() {
        let parsed = parse("Template: a/b\ndescription: Foo bar\n").expect("parsed");
        assert_eq!(template_name(&parsed[0]), "a/b");
        assert_eq!(short_description(&parsed[0]), "Foo bar");
        assert!(!known_field("description"));
    }
}
