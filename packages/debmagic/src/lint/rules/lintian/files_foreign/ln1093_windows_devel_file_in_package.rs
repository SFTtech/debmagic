use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::emit;

pub struct WindowsDevelFileInPackage;

declare_rule! {
    /// A Visual Studio project or solution file: `.vcproj`, `.sln`, `.dsp`
    /// or `.dsw`, optionally `.gz`.
    ///
    /// The name must contain a slash before the suffix with at least one
    /// character between them, so a root `foo.sln` is not flagged. Files
    /// under `usr/share/doc/` are skipped. Regular files and hardlinks only,
    /// matched on the raw bytes of the name. Udebs are checked too. Extra is
    /// empty.
    WindowsDevelFileInPackage,
    code = "LN1093",
    tag = "windows-devel-file-in-package",
    default_selected = true,
    experimental = false,
}

/// `m{/.+\.(?:vcproj|sln|ds[pw])(?:\.gz)?$}` on bytes; `(?-u)` keeps `.`
/// and `.+` byte-wise so non-UTF-8 names still match.
static DEVEL_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)/.+\.(?:vcproj|sln|ds[pw])(?:\.gz)?$").expect("valid regex")
});

const DOC_PREFIX: &[u8] = b"usr/share/doc/";

impl BinaryPackageRule for WindowsDevelFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                let name = entry.path().as_os_str().as_encoded_bytes();
                !name.starts_with(DOC_PREFIX) && DEVEL_FILE.is_match(name)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let text = TestFile::new("x");
        let pass = vec![
            file("foo.sln", text.clone()),
            file("a/.sln", text.clone()),
            file("usr/share/doc/foo/foo.sln", text.clone()),
            file("usr/share/doc/foo/foo.sln.gz", text.clone()),
            file("usr/lib/foo/foo.vcproj", TestFile::directory()),
            file("usr/lib/foo/foo.sln", TestFile::symlink("elsewhere")),
            file("usr/lib/foo/foo.SLN", text.clone()),
            file("usr/lib/foo/foo.dsp.bz2", text.clone()),
        ];
        let fail = vec![
            file("usr/lib/foo/foo.vcproj", text.clone()),
            file("usr/lib/.vcproj", text.clone()),
            file("usr/lib/foo.sln.gz", text.clone()),
            file("usr/lib/foo/foo.dsp", text.clone()),
            file("usr/lib/foo/foo.dsw.gz", text.clone()),
            file("usr/lib/foo/foo.sln", TestFile::new("x").hardlink()),
            file("usr/lib/foo/foo.dsw", text).udeb(),
        ];
        BinaryPackageTester::new(WindowsDevelFileInPackage, pass, fail).test_and_snapshot();
    }
}
