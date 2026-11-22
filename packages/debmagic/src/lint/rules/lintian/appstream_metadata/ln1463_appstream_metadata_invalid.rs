use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataInvalid;

declare_rule! {
    /// An AppStream metadata file does not consist of valid XML.
    ///
    /// Checked for the files in `usr/share/metainfo/` and below
    /// `usr/share/appdata/`. Not well-formed XML, or bytes that are not UTF-8,
    /// are reported. Extra is empty and the Location is the file. External
    /// entities are not loaded and DTDs are accepted, so the parser never
    /// reaches the network.
    AppstreamMetadataInvalid,
    code = "LN1463",
    tag = "appstream-metadata-invalid",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AppstreamMetadataInvalid {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.appstream_files() else {
            return;
        };
        for document in files.all() {
            if document.parse_error().is_none() {
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
            files(&[("usr/share/appdata/foo.xml", "<application/>")]),
            files(&[("usr/share/doc/foo/notes.txt", "not xml")]),
        ];
        let fail = vec![
            files(&[("usr/share/metainfo/foo.metainfo.xml", "<component>")]),
            files(&[("usr/share/appdata/foo.xml", "plain text")]),
            BinaryPackageCase::from(vec![(
                "usr/share/metainfo/bad.xml".to_string(),
                TestFile::from_bytes(b"<component>\xff</component>".to_vec()),
            )]),
        ];
        BinaryPackageTester::new(AppstreamMetadataInvalid, pass, fail).test_and_snapshot();
    }
}
