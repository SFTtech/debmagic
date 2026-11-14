use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::manual::{is_accepted_undocumented_target, section_page};

pub struct UndocumentedManualPage;

declare_rule! {
    /// A manual-page symlink points at `undocumented(7)`.
    ///
    /// The symlink is in a `usr/share/man/.../manN/` directory and its target
    /// matches `(^|/)undocumented` in a form the Check accepts: it starts
    /// with `undocumented.[237].gz` and the page directory starts with
    /// `usr/share/man/man` plus that section, or it is
    /// `../man[237]/undocumented.[237].gz`,
    /// `../../man/man[237]/undocumented.[237].gz`,
    /// `../../../share/man/man[237]/undocumented.[237].gz`, or
    /// `../../../../usr/share/man/man[237]/undocumented.[237].gz`. Any other
    /// `undocumented` target is `broken-link-to-undocumented`. Udebs are
    /// skipped. Extra is empty; the Data files path is the Location.
    UndocumentedManualPage,
    code = "LN0272",
    tag = "undocumented-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UndocumentedManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if entry.kind() != EntryKind::Symlink {
                continue;
            }
            let Some(page) = section_page(entry) else {
                continue;
            };
            let Some(target) = entry.link_target().and_then(|target| target.to_str()) else {
                continue;
            };
            if is_accepted_undocumented_target(target, &page.page_path) {
                hits.push(entry.path().to_path_buf());
            }
        }
        for path in hits {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn link(path: &str, target: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::symlink(target))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![link(
                "usr/share/man/man1/other.1.gz",
                "real-page.1.gz",
            )]),
            // Accepted by the other Tag: wrong section, absolute, or not a
            // known relative form.
            case(vec![
                link("usr/share/man/man1/sbin-binary.1.gz", "undocumented.7.gz"),
                link("usr/share/man/man1/section-one.1.gz", "undocumented.1.gz"),
                link("usr/share/man/man1/bare.1.gz", "undocumented"),
                link(
                    "usr/share/man/man7/absolute.7.gz",
                    "/usr/share/man/man7/undocumented.7.gz",
                ),
                link("usr/share/man/de/man7/locale.7.gz", "undocumented.7.gz"),
                link(
                    "usr/share/man/man1/relative-bad.1.gz",
                    "../man1/undocumented.1.gz",
                ),
            ]),
            case(vec![(
                "usr/share/man/man7/foo.7.gz".to_string(),
                TestFile::new("not a link"),
            )]),
            case(vec![link(
                "usr/share/man/odd/foo.1.gz",
                "undocumented.7.gz",
            )]),
            case(vec![link(
                "usr/share/man/man7/foo.7.gz",
                "undocumented.7.gz",
            )])
            .udeb(),
        ];
        let fail = vec![case(vec![
            link("usr/share/man/man7/foo.7.gz", "undocumented.7.gz"),
            link("usr/share/man/man2/foo.2.gz", "undocumented.2.gz"),
            link("usr/share/man/man3/foo.3.gz", "undocumented.3.gz"),
            link("usr/share/man/man7/extra.7.gz", "undocumented.7.gz.extra"),
            link("usr/share/man/man1/rel.1.gz", "../man2/undocumented.7.gz"),
            link(
                "usr/share/man/de/man1/rel.1.gz",
                "../man7/undocumented.7.gz",
            ),
            link(
                "usr/share/man/man1/up.1.gz",
                "../../man/man7/undocumented.7.gz",
            ),
            link(
                "usr/share/man/man1/share.1.gz",
                "../../../share/man/man2/undocumented.2.gz",
            ),
            link(
                "usr/share/man/man1/usr.1.gz",
                "../../../../usr/share/man/man3/undocumented.7.gz",
            ),
        ])];
        BinaryPackageTester::new(UndocumentedManualPage, pass, fail).test_and_snapshot();
    }
}
