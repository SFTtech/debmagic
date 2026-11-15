use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::lintian_tags::current_tag_name;
use crate::lint::rule::BinaryPackageRule;

pub struct AlienTag;

declare_rule! {
    /// A Lintian override names a Tag lintian does not know.
    ///
    /// One Diagnostic per override in the installed override file
    /// (`usr/share/lintian/overrides/<package>`, else `.gz`) whose Tag is
    /// neither a lintian 2.139.0 Tag nor an old name one was renamed from.
    /// An old name is `renamed-tag`, not this. Extra is the Tag as written
    /// (a `pkg:` prefix for another package stays part of it). The override
    /// file and line are the Location. Overrides are not applied.
    AlienTag,
    code = "LN0707",
    tag = "alien-tag",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AlienTag {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        for entry in overrides.overrides() {
            if current_tag_name(entry.tag()).is_some() {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(entry.tag()).with_location(Location {
                path: overrides.path().to_path_buf(),
                line: Some(entry.line()),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn overrides(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(content))])
    }

    fn installed(content: &str) -> BinaryPackageCase {
        overrides("usr/share/lintian/overrides/example", content)
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).expect("gzip in memory");
        encoder.finish().expect("gzip in memory")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            installed("example: copyright-contains-dh_make-todo-boilerplate foo-bar-extra\n"),
            installed("# justified\nexample binary: no-copyright-file\n[amd64]: elf-error\n"),
            // Old name: renamed-tag, not alien-tag.
            installed("package-install-apt-sources\n"),
            // Lines that do not parse are malformed-override.
            installed("example no-copyright-file\nexample:\n"),
            // Another package's override file.
            overrides("usr/share/lintian/overrides/other", "foo\n"),
            // A directory is override-file-in-wrong-location.
            overrides("usr/share/lintian/overrides/example/foo", "foo\n"),
            installed("example udeb: no-copyright-file\n").udeb(),
        ];
        let fail = vec![
            installed("foo\n"),
            installed("lintian-override: \nexample: copyright-contains-dh_make-todo-boilerplate\n"),
            installed("# why\n\nexample [amd64]: unknown-thing some context\nexample udeb: x\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/lintian/overrides/example.gz".to_string(),
                TestFile::from_bytes(gzip(b"zipped-mystery\n")),
            )]),
            installed("binary: no-copyright-file\n").udeb(),
            installed("no-git\nteam/pkg-perl/vcs/no-git\n"),
        ];
        BinaryPackageTester::new(AlienTag, pass, fail).test_and_snapshot();
    }
}
