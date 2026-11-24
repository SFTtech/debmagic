use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::files;

static GENERAL_LIST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i-u)Language-Team:.*debian-i18n@lists\.debian\.org").expect("valid regex")
});

pub struct DebconfTranslationUsingGeneralList;

declare_rule! {
    /// A translation in `debian/po` (or `debian/po.in`) names the general
    /// `debian-i18n@lists.debian.org` list in its `Language-Team` header.
    ///
    /// Only checked when the package uses po-debconf: some debconf template
    /// has a translatable description and the PO directory exists. Each
    /// `.po` file that is a regular file (or a symlink to one) is searched
    /// case-insensitively. Extra is empty. The Location is the `.po` file.
    DebconfTranslationUsingGeneralList,
    code = "LN0902",
    tag = "debconf-translation-using-general-list",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebconfTranslationUsingGeneralList {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        if !files::has_translatable_template(tree) {
            return;
        }
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        let paths: Vec<PathBuf> = files::po_children(tree, directory)
            .into_iter()
            .filter(|entry| {
                files::open_file(tree, entry.path())
                    .and_then(|target| tree.read(target).ok())
                    .is_some_and(|bytes| GENERAL_LIST.is_match(&bytes))
            })
            .map(files::path_buf)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const TEMPLATE: &str = "Template: example/foo\nType: string\nDescription: Ask\n Question.\n";
    const PO: &str = "msgid \"a\"\nmsgstr \"b\"\n";
    const GENERAL: &str = "msgid \"\"\nmsgstr \"\"\n\"Language-Team: Debian I18N <debian-i18n@lists.debian.org>\\n\"\n";
    const OWN_LIST: &str = "msgid \"\"\nmsgstr \"\"\n\"Language-Team: German <de@li.org>\\n\"\n";

    fn tree(po_name: &str, po: &str) -> Vec<(String, TestFile)> {
        vec![
            (
                "debian/control".to_string(),
                TestFile::from(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            ("debian/templates".to_string(), TestFile::new(TEMPLATE)),
            ("debian/po".to_string(), TestFile::directory()),
            (format!("debian/po/{po_name}"), TestFile::new(po)),
        ]
    }

    #[test]
    fn test() {
        let no_templates = vec![
            (
                "debian/control".to_string(),
                TestFile::from("Source: example\n"),
            ),
            ("debian/po".to_string(), TestFile::directory()),
            ("debian/po/en_US.po".to_string(), TestFile::new(GENERAL)),
        ];
        let pass = vec![tree("de.po", OWN_LIST), tree("en_US.po", PO), no_templates];
        let fail = vec![
            tree("en_US.po", GENERAL),
            tree("de.po", &GENERAL.replace("debian-i18n@", "DEBIAN-I18N@")),
        ];
        SourceTreeTester::new(DebconfTranslationUsingGeneralList, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("de.po", OWN_LIST))];
        let fail = vec![SourcePackageCase::from(tree("en_US.po", GENERAL))];
        SourcePackageTester::new(DebconfTranslationUsingGeneralList, pass, fail)
            .test_and_snapshot();
    }
}
