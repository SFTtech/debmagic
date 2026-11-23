use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct UnpackMessageForOrig;

declare_rule! {
    /// Unpacking the contents of this source package produced the listed
    /// error. It probably means something is broken or the package was
    /// somehow constructed in a strange way. You may wish to report this as a
    /// bug to upstream.
    ///
    /// Reports each line tar printed while it extracted an Orig tarball,
    /// except lintian's `tar: Ignoring ` lines, once per message per tarball.
    /// The extra is `<tarball> . <message>`, as lintian words it, for example
    /// `example_1.0.orig.tar.gz . tar: Removing leading `/' from member names`.
    /// Extraction keeps the header time stamps off (`tar --touch`, as lintian
    /// does), so tar does not warn about ancient ones here; those are the
    /// Patched files' messages in `UnpackMessageForSource`. A Native package
    /// has no Orig tarball and no message; a Source tree has none either.
    UnpackMessageForOrig,
    code = "LN1799",
    tag = "unpack-message-for-orig",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnpackMessageForOrig {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx.orig_unpack_messages().to_vec();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use crate::lint::context::{SourceContext, SourcePackageData};
    use crate::lint::source_package::{write_test_native_dsc, write_test_quilt_dsc};
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    /// The messages of one lint of the Source package `dsc`.
    fn messages(dsc: PathBuf) -> Vec<String> {
        let data = SourcePackageData::new(dsc).expect("Source package");
        let mut ctx = SourceContext::bind_package(&data, &UnpackMessageForOrig);
        UnpackMessageForOrig.run(&mut ctx);
        ctx.finish()
            .expect("no runtime error")
            .into_iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    /// The `.dsc` of a quilt package with `orig` members in `dir`.
    fn quilt(dir: &Path, orig: &[(&str, &[u8], u64)]) -> PathBuf {
        write_test_quilt_dsc(dir, "example", orig).expect("quilt dsc")
    }

    #[test]
    fn leading_slash_in_orig_is_reported_with_its_tarball() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(
            dir.path(),
            &[("example-1.0/README", b"text\n", 0), ("/abs/file", b"x", 0)],
        );
        assert_eq!(
            messages(dsc),
            ["example_1.0.orig.tar.gz . tar: Removing leading `/' from member names"]
        );
    }

    #[test]
    fn clean_orig_reports_nothing() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(dir.path(), &[("example-1.0/README", b"text\n", 0)]);
        assert!(messages(dsc).is_empty());
    }

    #[test]
    fn native_package_has_no_orig_messages() {
        let dir = TempDir::new().expect("temp directory");
        let dsc =
            write_test_native_dsc(dir.path(), "example", &[("README", b"text\n")]).expect("dsc");
        assert!(messages(dsc).is_empty());
    }

    #[test]
    fn source_tree_has_no_orig_messages() {
        let pass = Vec::<TestCase>::new();
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(UnpackMessageForOrig, pass, fail).test();
    }
}
