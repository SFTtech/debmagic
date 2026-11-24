use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;
use crate::lint::upstream_metadata::{UPSTREAM_METADATA_PATH, UpstreamMetadata};

pub struct UpstreamMetadataMissingRepository;

declare_rule! {
    /// The upstream metadata names no repository.
    ///
    /// Non-native packages only. Fires when the top-level mapping of
    /// `debian/upstream/metadata` has neither `Repository` nor `Repository-Browse`
    /// with a value (a null value counts as absent). Severity info; extra is
    /// empty; the path is the Location. Not reported when the file is missing,
    /// unreadable, invalid YAML, or not a mapping.
    UpstreamMetadataMissingRepository,
    code = "LN1340",
    tag = "upstream-metadata-missing-repository",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UpstreamMetadataMissingRepository {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(metadata) = ctx.upstream_metadata() else {
            return;
        };
        let UpstreamMetadata::Mapping(mapping) = metadata.as_ref() else {
            return;
        };
        if !(mapping.is_defined("Repository") || mapping.is_defined("Repository-Browse")) {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: Path::new(UPSTREAM_METADATA_PATH).to_path_buf(),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(
        version: &str,
        format: Option<&str>,
        extra: &[(&'static str, TestFile)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format)));
        }
        files.extend(extra.iter().cloned());
        files
    }

    fn metadata(text: &'static str) -> (&'static str, TestFile) {
        ("debian/upstream/metadata", TestFile::new(text))
    }

    fn package(
        format: &str,
        version: &str,
        dsc_files: &[&str],
        extra: Vec<(&'static str, TestFile)>,
    ) -> SourcePackageCase {
        let mut text = format!(
            "Format: {format}\nSource: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n"
        );
        for file in dsc_files {
            text.push_str(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {file}\n"));
        }
        let dsc = Dsc::parse(&text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        files.extend(extra);
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[metadata("Repository: x\n")]),
            tree(
                "1.0-1",
                None,
                &[metadata("Repository-Browse: x\nName: y\n")],
            ),
            tree(
                "1.0-1",
                None,
                &[metadata("Repository: x\nRepository-Browse: y\n")],
            ),
            tree(
                "1.0-1",
                None,
                &[metadata("Repository: ~\nRepository-Browse: y\n")],
            ),
            tree("1.0-1", None, &[metadata("- Name: x\n")]),
            tree("1.0-1", None, &[metadata("Name: [\n")]),
            tree("1.0-1", None, &[metadata("~\n")]),
            tree("1.0-1", None, &[]),
            tree("1.0", Some("3.0 (native)"), &[metadata("Name: x\n")]),
        ];
        let fail = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[metadata("Name: x\n")]),
            tree(
                "1.0-1",
                None,
                &[metadata("Repository: ~\nRepository-Browse:\n")],
            ),
        ];
        SourceTreeTester::new(UpstreamMetadataMissingRepository, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package(
                "3.0 (quilt)",
                "1.0-1",
                &["example_1.0.orig.tar.gz"],
                vec![metadata("Repository: x\n")],
            ),
            package(
                "3.0 (native)",
                "1.0",
                &["example_1.0.tar.gz"],
                vec![metadata("Name: x\n")],
            ),
        ];
        let fail = vec![package(
            "1.0",
            "1.0-1",
            &["example_1.0-1.diff.gz"],
            vec![metadata("Name: x\n")],
        )];
        SourcePackageTester::new(UpstreamMetadataMissingRepository, pass, fail).test_and_snapshot();
    }
}
