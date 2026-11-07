use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct UntranslatableDebconfTemplates;

declare_rule! {
    /// A debconf templates file under `debian/` has an untranslatable Description.
    ///
    /// Every direct child of `debian/` whose name is `templates`, `X.templates`,
    /// or `templates.Y` (`Y` may be longer) is read line by line, default and
    /// language files alike. A line `Description: text` (any case of the key,
    /// `text` non-empty) is reported unless `text` contains `for internal use`.
    /// Extra is empty. Location is that file and line.
    UntranslatableDebconfTemplates,
    code = "LN0375",
    tag = "untranslatable-debconf-templates",
    default_selected = true,
    experimental = false,
}

static TEMPLATES_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.+\.)?templates(?:\..+)?$").expect("valid regex"));

static DESCRIPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Description: (.+)").expect("valid regex"));

impl SourceRule for UntranslatableDebconfTemplates {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hits = match ctx.files() {
            Some(files) => untranslatable_lines(files),
            None => return,
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn untranslatable_lines(files: &FileTree) -> Vec<(PathBuf, u32)> {
    let mut hits = Vec::new();
    for entry in files.entries() {
        if !is_templates_child(entry) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for (index, line) in text.split('\n').enumerate() {
            if is_untranslatable(line) {
                let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                hits.push((entry.path().to_path_buf(), number));
            }
        }
    }
    hits
}

fn is_templates_child(entry: &Entry) -> bool {
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let path = entry.path();
    if path.parent() != Some(Path::new("debian")) {
        return false;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| TEMPLATES_NAME.is_match(name))
}

fn is_untranslatable(line: &str) -> bool {
    DESCRIPTION
        .captures(line)
        .is_some_and(|caps| !caps[1].contains("for internal use"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(path: &'static str, body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new("Source: example\n")),
            (path, TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(
                "debian/templates",
                "Template: a/b\nType: string\nDescription: for internal use\n",
            ),
            tree(
                "debian/templates",
                "Template: a/b\nType: string\n_Description: Translatable\n",
            ),
            tree("debian/xtemplates", "Description: Not a templates file\n"),
            tree("debian/templates", "Template: a/b\nDescription:\n"),
            tree(
                "debian/tests/templates",
                "Description: Not a direct child\n",
            ),
        ];
        let fail = vec![
            tree(
                "debian/templates",
                "Template: a/b\nType: string\nDescription: Ask me\n",
            ),
            tree(
                "debian/templates.fr",
                "Template: a/b\nDescription: Pose une question\n",
            ),
            tree(
                "debian/example.templates",
                "Template: a\ndescription: lower case key\n",
            ),
        ];
        SourceTreeTester::new(UntranslatableDebconfTemplates, pass, fail).test_and_snapshot();
    }
}
