use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct VimAddonWithinVimRuntimePath;

declare_rule! {
    /// An Entry of the Binary package sits directly under a Vim runtime directory,
    /// `usr/share/vim/vimcurrent/<name>` or `usr/share/vim/vim<NN>/<name>`, where
    /// `<name>` is the first component below it. Extra is empty; the Entry's path
    /// is the Location.
    ///
    /// Skipped when the Source package name contains `vim` (the Vim packages
    /// themselves), and for the `doc` component of a `vimhelp-<lang>` Binary
    /// package. Directories, symlinks and udebs are judged like files, as in
    /// lintian's `visit_installed_files`. The names are matched as raw bytes.
    VimAddonWithinVimRuntimePath,
    code = "LN1147",
    tag = "vim-addon-within-vim-runtime-path",
    default_selected = true,
    experimental = false,
}

static RUNTIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)^usr/share/vim/vim(?:current|[0-9]{2})/([^/]+)").expect("valid regex")
});

static VIMHELP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^vimhelp-[A-Za-z0-9_]+$").expect("valid regex"));

impl BinaryPackageRule for VimAddonWithinVimRuntimePath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let is_vim = ctx.source_name().contains("vim");
        let is_vimhelp_package = VIMHELP.is_match(ctx.package_name().as_bytes());
        if is_vim {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                let bytes = entry.path().as_os_str().as_encoded_bytes();
                RUNTIME.captures(bytes).is_some_and(|captures| {
                    let component = captures.get(1).map_or(&b""[..], |m| m.as_bytes());
                    !(component == b"doc" && is_vimhelp_package)
                })
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Vim's own packages, judged by the Source package name.
            files(&["usr/share/vim/vimcurrent/bar"]).field("Source", "vim (2:9.0-1)"),
            files(&["usr/share/vim/vim73/bar"]).field("Source", "neovim-vim (1.0-1)"),
            // The `doc` directory of a vimhelp package.
            files(&["usr/share/vim/vimcurrent/doc/tags"]).package("vimhelp-en"),
            // Not directly under a runtime directory.
            files(&[
                "usr/share/vim/addons/foo.vim",
                "usr/share/vim/vimcurrent",
                "usr/share/vim/vimfoo/bar",
                "usr/share/vim/vim7/bar",
                "usr/share/vim/vimcurrent-x/bar",
                "other/usr/share/vim/vimcurrent/bar",
                "usr/share/vim/bar",
            ]),
            files(&["usr/share/doc/example/vim/bar"]),
        ];
        let fail = vec![
            files(&["usr/share/vim/vimcurrent/bar"]),
            files(&["usr/share/vim/vim73/bar"]),
            files(&["usr/share/vim/vimcurrent/bar/baz"]),
            // Only the `doc` component is exempt, and only for vimhelp packages.
            files(&["usr/share/vim/vimcurrent/doc/tags"]),
            files(&["usr/share/vim/vimcurrent/bar"])
                .package("vimhelp-en")
                .field("Source", "example"),
            files(&["usr/share/vim/vimcurrent/doc/tags"]).field("Source", "example"),
            // A directory and a symlink are judged too.
            BinaryPackageCase::from(vec![(
                "usr/share/vim/vimcurrent/bar",
                TestFile::directory(),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/vim/vimcurrent/bar",
                TestFile::symlink("/usr/share/foo"),
            )]),
            // A udeb is not skipped.
            files(&["usr/share/vim/vimcurrent/bar"]).udeb(),
            // A name that is not UTF-8 is matched by its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/vim/vimcurrent/bokm\xe5l".to_vec()),
                TestFile::new("content"),
            )]),
        ];
        BinaryPackageTester::new(VimAddonWithinVimRuntimePath, pass, fail).test_and_snapshot();
    }
}
