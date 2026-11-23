use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;
use crate::lint::upstream_metadata::UpstreamMetadata;

/// Where salsa-ci's configuration may be (lintian's `@KNOWN_LOCATIONS`).
const KNOWN_LOCATIONS: &[&str] = &[
    "debian/salsa-ci.yml",
    "debian/gitlab-ci.yml",
    "debian/.gitlab-ci.yml",
    "gitlab-ci.yml",
    ".gitlab-ci.yml",
];

pub struct InvalidYaml;

declare_rule! {
    /// A salsa-ci configuration file is not valid YAML.
    ///
    /// Checked on the packaging File tree (Source tree or Patched files) at the
    /// five locations salsa-ci reads: `debian/salsa-ci.yml`,
    /// `debian/gitlab-ci.yml`, `debian/.gitlab-ci.yml`, `gitlab-ci.yml` and
    /// `.gitlab-ci.yml`. The file is parsed with the same YAML reader as the
    /// upstream metadata Check, so a syntax error, invalid UTF-8, and a file with
    /// no document or only a null one (which `YAML::XS` loads as undef) are all
    /// reported. Only regular files and hardlinks are read, as lintian's
    /// `is_file`: a symlink or a directory at those paths is not reported. The
    /// Diagnostic has an empty extra and the file as its Location, with no line,
    /// as lintian's pointed hint has no extra. A file that cannot be read is
    /// skipped.
    InvalidYaml,
    code = "LN1788",
    tag = "invalid-yaml",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidYaml {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut invalid = Vec::<PathBuf>::new();
        for entry in files.entries() {
            if !entry.is_file()
                || !KNOWN_LOCATIONS
                    .iter()
                    .any(|location| entry.path() == Path::new(location))
            {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if matches!(
                UpstreamMetadata::parse(&bytes),
                UpstreamMetadata::Invalid(_) | UpstreamMetadata::Empty
            ) {
                invalid.push(entry.path().to_path_buf());
            }
        }
        for path in invalid {
            ctx.diagnostic(Diagnostic::warn(String::new()).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{
        SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase, TestFile,
    };

    use super::*;

    const DSC: &str = "Format: 3.0 (quilt)\nSource: example\nBinary: example\n\
         Architecture: all\nVersion: 1.0-1\nMaintainer: Example <ex@example.com>\n\
         Files:\n d41d8cd98f00b204e9800998ecf8427e 0 example_1.0.orig.tar.gz\n";

    const VALID: &str = "include:\n  - local: debian/ci.yml\nstages: [build]\n";
    const INVALID: &str = "include: [\n";

    fn tree(path: &'static str, file: TestFile) -> TestCase {
        TestCase::source_tree([(path, file)])
    }

    fn package(path: &'static str, file: TestFile) -> SourcePackageCase {
        SourcePackageCase::from(vec![(path, file)]).dsc(Dsc::parse(DSC).expect("valid dsc"))
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            tree("debian/salsa-ci.yml", TestFile::new(VALID)),
            tree("gitlab-ci.yml", TestFile::new(VALID)),
            // Not a known location, even when invalid.
            tree("debian/salsa-ci.yaml", TestFile::new(INVALID)),
            tree("debian/rules", TestFile::new(INVALID)),
            // Only the regular file or hardlink is read; a symlink is not.
            TestCase::source_tree([
                ("debian/other.yml", TestFile::new(INVALID)),
                ("debian/salsa-ci.yml", TestFile::symlink("other.yml")),
            ]),
            tree("debian/salsa-ci.yml", TestFile::directory()),
            // A file with no document in another name is not checked.
            tree("debian/gitlab-ci.yml.bak", TestFile::new("")),
            TestCase::source_tree([("debian/control", TestFile::new("Source: example\n"))]),
        ];
        let fail = vec![
            tree("debian/salsa-ci.yml", TestFile::new(INVALID)),
            tree("debian/gitlab-ci.yml", TestFile::new("a: 'unterminated\n")),
            tree("debian/.gitlab-ci.yml", TestFile::new(INVALID)),
            tree("gitlab-ci.yml", TestFile::new(INVALID)),
            tree(".gitlab-ci.yml", TestFile::new(INVALID)),
            // A hardlink is a file.
            tree("debian/salsa-ci.yml", TestFile::new(INVALID).hardlink()),
            // An empty file and a null document load as undef in YAML::XS.
            tree("debian/salsa-ci.yml", TestFile::new("")),
            tree("debian/salsa-ci.yml", TestFile::new("# only a comment\n")),
            tree("debian/gitlab-ci.yml", TestFile::new("~\n")),
            // Invalid UTF-8.
            tree(
                "gitlab-ci.yml",
                TestFile::from_bytes(b"key: \xff\n".to_vec()),
            ),
        ];
        SourceTreeTester::new(InvalidYaml, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package("debian/salsa-ci.yml", TestFile::new(VALID)),
            package("debian/other.yml", TestFile::new(INVALID)),
        ];
        let fail = vec![
            package("debian/salsa-ci.yml", TestFile::new(INVALID)),
            package("debian/gitlab-ci.yml", TestFile::new("")),
        ];
        SourcePackageTester::new(InvalidYaml, pass, fail).test_and_snapshot();
    }
}
