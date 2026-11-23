use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct UnpackMessageForSource;

declare_rule! {
    /// Unpacking the contents of this source package produced the listed
    /// error. It probably means something is broken or the package was
    /// somehow constructed in a strange way.
    ///
    /// Reports each line `dpkg-source` printed while extracting the Patched
    /// files, except lintian's `tar: Ignoring ` lines. Those lines include
    /// tar's warnings about the Orig tarballs, which `dpkg-source` unpacks
    /// itself: for example `tar: ancient-source-1.0/README: implausibly old
    /// time stamp`. The extra is the line, and the Rule is info, not
    /// selected by default. A Source tree has no unpack messages. The Orig
    /// tarballs' messages are `UnpackMessageForOrig`.
    UnpackMessageForSource,
    code = "LN1711",
    tag = "unpack-message-for-source",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UnpackMessageForSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx.patched_unpack_messages().to_vec();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    use tempfile::TempDir;

    use crate::lint::context::{SourceContext, SourcePackageData};
    use crate::lint::source_package::{
        build_test_native_dsc, write_test_native_dsc, write_test_native_tree,
    };
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    /// The messages of one lint of the Source package `dsc`.
    fn messages(dsc: PathBuf) -> Vec<String> {
        let data = SourcePackageData::new(dsc).expect("Source package");
        let mut ctx = SourceContext::bind_package(&data, &UnpackMessageForSource);
        UnpackMessageForSource.run(&mut ctx);
        ctx.finish()
            .expect("no runtime error")
            .into_iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    /// Sets `path`'s modification time, which may be before the epoch.
    fn set_mtime(path: &Path, mtime: SystemTime) {
        File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(mtime))
            .expect("setting the modification time");
    }

    #[test]
    fn ancient_file_in_patched_tree_is_reported() {
        let dir = TempDir::new().expect("temp directory");
        let tree =
            write_test_native_tree(dir.path(), "example", &[("README", b"text\n")]).expect("tree");
        set_mtime(
            &tree.join("README"),
            SystemTime::UNIX_EPOCH - Duration::from_secs(1),
        );
        let dsc = build_test_native_dsc(dir.path(), "example").expect("dsc");
        let found = messages(dsc);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("tar: "), "{found:?}");
        assert!(found[0].contains("README"), "{found:?}");
        assert!(found[0].contains("implausibly old time stamp"), "{found:?}");
    }

    #[test]
    fn clean_package_reports_nothing() {
        let dir = TempDir::new().expect("temp directory");
        let dsc =
            write_test_native_dsc(dir.path(), "example", &[("README", b"text\n")]).expect("dsc");
        assert!(messages(dsc).is_empty());
    }

    #[test]
    fn source_tree_has_no_unpack_messages() {
        let pass = Vec::<TestCase>::new();
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(UnpackMessageForSource, pass, fail).test();
    }
}
