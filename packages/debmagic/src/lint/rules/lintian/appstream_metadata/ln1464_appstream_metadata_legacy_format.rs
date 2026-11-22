use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataLegacyFormat;

declare_rule! {
    /// An AppStream metadata file uses the obsolete `<application>` root.
    ///
    /// Checked for the files in `usr/share/metainfo/` and below
    /// `usr/share/appdata/` that parse as XML. A root element named
    /// `application` is the legacy format. Extra is empty and the Location is
    /// the file. Namespaced roots are not matched.
    AppstreamMetadataLegacyFormat,
    code = "LN1464",
    tag = "appstream-metadata-legacy-format",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AppstreamMetadataLegacyFormat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.appstream_files() else {
            return;
        };
        for document in files.all() {
            if document.root_element() != Some("application") {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: document.path().to_path_buf(),
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

    fn files(paths: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&[("usr/share/metainfo/foo.metainfo.xml", "<component/>")]),
            files(&[("usr/share/doc/foo.xml", "<application/>")]),
        ];
        let fail = vec![
            files(&[(
                "usr/share/appdata/foo.xml",
                "<application><id>x</id></application>",
            )]),
            files(&[("usr/share/metainfo/foo.metainfo.xml", "<application/>")]),
        ];
        BinaryPackageTester::new(AppstreamMetadataLegacyFormat, pass, fail).test_and_snapshot();
    }
}
