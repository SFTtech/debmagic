use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct StrayTranslatedDebconfTemplates;

declare_rule! {
    /// A `templates.LL` file remains while the package uses po-debconf.
    ///
    /// A direct child of `debian/` whose name matches `templates` with an
    /// optional prefix and suffix is a template file. `templates.LL` and
    /// `templates.LL_CC` are translated copies. They are reported when some
    /// other template file declares a `Template` that is not a shared word
    /// list and a `Description` that is not "for internal use", and
    /// `debian/po` or `debian/po.in` is a directory. A name ending in
    /// `templates.in` is kept. Extra is empty. The Location is the file.
    /// Applies to the Source tree and Source package.
    StrayTranslatedDebconfTemplates,
    code = "LN0407",
    tag = "stray-translated-debconf-templates",
    default_selected = true,
    experimental = false,
}

static TEMPLATES_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+\.)?templates(\..+)?$").expect("regex"));
static LANG_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"templates\.\w\w(_\w\w)?$").expect("regex"));
static KEPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"templates\.in$").expect("regex"));
static TEMPLATE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Template:\s+(\S+)").expect("regex"));
static DESCRIPTION_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^_?Description:\s+(.+)").expect("regex"));

impl SourceRule for StrayTranslatedDebconfTemplates {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if !uses_po_debconf(files) {
            return;
        }
        let mut paths = stray(files);
        paths.sort();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn uses_po_debconf(files: &FileTree) -> bool {
    has_template(files) && po_dir(files)
}

fn po_dir(files: &FileTree) -> bool {
    if is_dir(files, "debian/po") {
        return true;
    }
    is_dir(files, "debian/po.in")
}

fn is_dir(files: &FileTree, path: &str) -> bool {
    files
        .get(path)
        .is_some_and(|entry| entry.kind() == EntryKind::Directory)
}

fn has_template(files: &FileTree) -> bool {
    debian_children(files).into_iter().any(|(path, bytes)| {
        let Some(name) = file_name(&path) else {
            return false;
        };
        TEMPLATES_NAME.is_match(name) && !LANG_NAME.is_match(name) && template_body(&bytes)
    })
}

fn stray(files: &FileTree) -> Vec<PathBuf> {
    debian_children(files)
        .into_iter()
        .filter_map(|(path, _)| {
            let name = file_name(&path)?;
            (LANG_NAME.is_match(name) && !KEPT.is_match(name)).then_some(path)
        })
        .collect()
}

fn debian_children(files: &FileTree) -> Vec<(PathBuf, Vec<u8>)> {
    files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && entry.path().parent() == Some(Path::new("debian"))
        })
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            Some((entry.path().to_path_buf(), bytes))
        })
        .collect()
}

fn file_name(path: &Path) -> Option<&str> {
    path.file_name()?.to_str()
}

fn template_body(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let mut in_template = false;
    for line in text.split('\n') {
        if let Some(captures) = TEMPLATE_LINE.captures(line) {
            let template = &captures[1];
            if template == "shared/packages-wordlist"
                || template == "shared/packages-ispell"
                || template.ends_with("/languages")
            {
                continue;
            }
            in_template = true;
        } else if in_template && let Some(captures) = DESCRIPTION_LINE.captures(line) {
            if !captures[1].contains("for internal use") {
                return true;
            }
        } else if in_template && line.is_empty() {
            in_template = false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const TEMPLATE: &str = "\
Template: example/test
Type: string
_Default: foo
Description: This is a string question used for testing:
";

    fn packaged(extra: Vec<(&str, TestFile)>) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/templates".to_string(), TestFile::new(TEMPLATE)),
            ("debian/po".to_string(), TestFile::directory()),
            (
                "debian/po/templates.pot".to_string(),
                TestFile::new("msgid\n"),
            ),
        ];
        for (path, file) in extra {
            files.push((path.to_string(), file));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            packaged(vec![]),
            packaged(vec![(
                "debian/templates.in",
                TestFile::new("Template: x\n"),
            )]),
            vec![
                ("debian/templates".to_string(), TestFile::new(TEMPLATE)),
                (
                    "debian/templates.de".to_string(),
                    TestFile::new("Template: x\n"),
                ),
            ],
        ];
        let fail = vec![
            packaged(vec![(
                "debian/templates.en_US",
                TestFile::new("Template: x\nDescription: y\n"),
            )]),
            packaged(vec![(
                "debian/debconf-test.templates.de",
                TestFile::new("translated\n"),
            )]),
        ];
        SourceTreeTester::new(StrayTranslatedDebconfTemplates, pass, fail).test_and_snapshot();
    }
}
