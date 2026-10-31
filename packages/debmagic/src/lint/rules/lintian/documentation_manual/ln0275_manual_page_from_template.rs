use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content;

pub struct ManualPageFromTemplate;

declare_rule! {
    /// The whatis entry is still the dh_make template.
    ///
    /// A regular file or hardlink under `usr/share/man/manN/`. `lexgrog` must
    /// exit 0 or 2. The description, after stripping a leading `path: "..."`,
    /// matches `programs? to do something` and does not match the useless
    /// whatis pattern. A page under 256 bytes whose opening line is empty or
    /// a `.so` request is left alone. Locale directories are not scanned.
    /// Udebs are skipped. Extra is empty; the Data files path is the Location.
    ManualPageFromTemplate,
    code = "LN0275",
    tag = "manual-page-from-template",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ManualPageFromTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for page in content::pages(files) {
            if page.opening.stops() || !page.english_hierarchy {
                continue;
            }
            let Some(whatis) = content::whatis(&page) else {
                continue;
            };
            if whatis.template() {
                hits.push(page.path);
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const TEMPLATE: &str = ".TH PROGRAM 1\n.SH NAME\nprogram \\- program to do something\n";
    const PLURAL: &str = ".TH PROGRAM 1\n.SH NAME\nfoo \\- programs to do something\n";
    const USELESS: &str = ".TH TRUE 1\n.SH NAME\ntrue \\- manual page for true 5.96\n";
    const CLEAN: &str = ".TH FOO 1\n.SH NAME\nfoo \\- frobnicate files\n";

    fn text(path: &str, body: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(body))
    }

    fn gzip(path: &str, body: &str) -> (String, TestFile) {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        (
            path.to_string(),
            TestFile::from_bytes(encoder.finish().expect("gzip")),
        )
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/clean.1", CLEAN)]),
            // Useless wins over the template pattern when both could apply.
            case(vec![text("usr/share/man/man1/true.1", USELESS)]),
            case(vec![text("usr/share/man/de/man1/program.1", TEMPLATE)]),
            case(vec![text("usr/share/man/man1/link.1", ".so man1/foo.1\n")]),
            case(vec![text("usr/share/man/man1/program.1", TEMPLATE)]).udeb(),
        ];
        let fail = vec![case(vec![
            text("usr/share/man/man1/program.1", TEMPLATE),
            text("usr/share/man/man1/plural.1", PLURAL),
            gzip("usr/share/man/man1/program.1.gz", TEMPLATE),
        ])];
        BinaryPackageTester::new(ManualPageFromTemplate, pass, fail).test_and_snapshot();
    }
}
