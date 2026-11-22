use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataInLegacyLocation;

declare_rule! {
    /// An AppStream metadata file is in the legacy `usr/share/appdata/`.
    ///
    /// Every file below `usr/share/appdata/` is reported, whatever its
    /// content. Metadata belongs in `usr/share/metainfo/`. Extra is empty and
    /// the Location is the file.
    AppstreamMetadataInLegacyLocation,
    code = "LN1462",
    tag = "appstream-metadata-in-legacy-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AppstreamMetadataInLegacyLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.appstream_files() else {
            return;
        };
        for document in files.legacy() {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("<component/>")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["usr/share/metainfo/foo.metainfo.xml"]),
            files(&["usr/share/doc/foo/appdata/foo.xml"]),
        ];
        let fail = vec![
            files(&["usr/share/appdata/foo.metadata.xml"]),
            files(&["usr/share/appdata/sub/foo.metadata.xml"]),
        ];
        BinaryPackageTester::new(AppstreamMetadataInLegacyLocation, pass, fail).test_and_snapshot();
    }
}
