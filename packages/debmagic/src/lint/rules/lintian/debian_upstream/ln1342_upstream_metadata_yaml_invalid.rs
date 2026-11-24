use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::fields_version::is_native;
use crate::lint::upstream_metadata::{UPSTREAM_METADATA_PATH, UpstreamMetadata};

pub struct UpstreamMetadataYamlInvalid;

declare_rule! {
    /// The upstream metadata is not valid YAML.
    ///
    /// Non-native packages only. Fires when `debian/upstream/metadata` does not
    /// parse as YAML, with the error worded and positioned as YAML::XS does:
    /// `<problem> (at document N, line L, column C)`. The error is in the extra;
    /// the path is the Location. A file with no document, or only a null one, is
    /// reported too, with the extra `document is empty` (lintian's extra there is
    /// a Perl `die` message, which is not reproduced). The document number counts
    /// an error found between two documents as in the later one, as libyaml does.
    UpstreamMetadataYamlInvalid,
    code = "LN1342",
    tag = "upstream-metadata-yaml-invalid",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UpstreamMetadataYamlInvalid {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(metadata) = ctx.upstream_metadata() else {
            return;
        };
        let extra = match metadata.as_ref() {
            UpstreamMetadata::Invalid(error) => error.to_string(),
            UpstreamMetadata::Empty => "document is empty".to_owned(),
            _ => return,
        };
        ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
            path: Path::new(UPSTREAM_METADATA_PATH).to_path_buf(),
            line: None,
            column: None,
        }));
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
            tree("1.0-1", None, &[metadata("Name: x\n")]),
            tree("1.0-1", None, &[metadata("- Name: x\n")]),
            tree("1.0-1", None, &[metadata("plain\n")]),
            tree("1.0-1", None, &[]),
            tree(
                "1.0",
                Some("3.0 (native)"),
                &[metadata(" - Hi,\nThis is invalid.\n")],
            ),
        ];
        let fail = vec![
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[metadata(
                    " - Hi,\nThis is a completely invalid YAML file.\n",
                )],
            ),
            tree("1.0-1", None, &[metadata("Name: x\nBad: [unclosed\n")]),
            tree("1.0-1", None, &[metadata("")]),
            tree("1.0-1", None, &[metadata("~\n")]),
        ];
        SourceTreeTester::new(UpstreamMetadataYamlInvalid, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![package(
            "3.0 (quilt)",
            "1.0-1",
            &["example_1.0.orig.tar.gz"],
            vec![metadata("Name: x\n")],
        )];
        let fail = vec![package(
            "1.0",
            "1.0-1",
            &["example_1.0-1.diff.gz"],
            vec![metadata(
                " - Hi,\nThis is a completely invalid YAML file.\n",
            )],
        )];
        SourcePackageTester::new(UpstreamMetadataYamlInvalid, pass, fail).test_and_snapshot();
    }
}
