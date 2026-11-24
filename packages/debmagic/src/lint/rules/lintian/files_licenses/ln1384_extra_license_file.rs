use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::entry_bytes;
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ExtraLicenseFile;

declare_rule! {
    /// A file whose basename looks like a license file (`COPYING`,
    /// `LICENSE`, `licence`, `LGPL`, `BSD`, `artistic`, with an optional
    /// version or suffix) ships outside `debian/copyright`'s reach.
    ///
    /// Covers regular files, directories, and other entries that are not
    /// symlinks or hardlinks. Skipped: executables, names with a source or
    /// picture extension (`.el`, `.c`, `.html`, `.png`, ...), Zope, KDE docbook,
    /// example, man page, pyshared-data, Cargo registry, GNOME help, and
    /// `common-licenses` paths, Sphinx `_sources`, and ELF or `ar` archive
    /// content. An ELF is recognised from the File type; the `ar` archive
    /// from its `!<arch>` magic. Extra is empty; the path is the Location.
    /// Severity is info and the Tag is not selected by default, as in lintian.
    ExtraLicenseFile,
    code = "LN1384",
    tag = "extra-license-file",
    default_selected = false,
    experimental = false,
}

/// The basename of a license file, case-insensitively.
static BASENAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:copying|licen[cs]e|l?gpl|bsd|artistic)[v0-9._-]*(?:\..*)?$")
        .expect("valid regex")
});

/// Source, picture and other extensions that are not license files.
static NON_LICENSE_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\.(?:el|[ch]|cc|p[ylmc]|[hu]i|p_hi|html|php|rb|xpm|png|jpe?g|gif|svg|dtd|mk|lisp|yml|rs|ogg|xbm)$",
    )
    .expect("valid regex")
});

/// Paths lintian leaves alone, as one pattern each.
static EXCLUDED_PATHS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^usr/share/zope/Products/.*\.(?:dtml|pt|cpt)$",
        r"/under\S+License\.docbook$",
        r"^usr/share/doc/[^/]+/examples/",
        r"^usr/share/man/(?:[^/]+/)?man\d/",
        r"^usr/share/pyshared-data/",
        r"^usr/share/cargo/registry/",
        r"^usr/share/(?:gnome/)?help/[^/]+/[^/]+/license\.page$",
        r"^usr/share/common-licenses/[^/]+$",
        r"(?i)/_sources/license(?:\.rst)?\.txt$",
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});

impl BinaryPackageRule for ExtraLicenseFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| is_extra_license_file(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_extra_license_file(files: &FileTree, entry: &Entry) -> bool {
    let Some(name) = entry.path().to_str() else {
        return false;
    };
    let Some(basename) = entry.path().file_name().and_then(|base| base.to_str()) else {
        return false;
    };
    if !BASENAME.is_match(basename)
        || entry.is_executable()
        || entry.link_target().is_some()
        || entry.hardlink_target().is_some()
        || NON_LICENSE_EXTENSION.is_match(name)
        || EXCLUDED_PATHS.iter().any(|pattern| pattern.is_match(name))
    {
        return false;
    }
    let elf = matches!(entry.file_type(), Some(FileType::Elf { .. }));
    !(elf || is_ar_archive(files, entry))
}

/// A file that `file(1)` calls a `current ar archive`: the `!<arch>` magic.
fn is_ar_archive(files: &FileTree, entry: &Entry) -> bool {
    entry_bytes(files, entry).is_some_and(|bytes| bytes.starts_with(b"!<arch>\n"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![("usr/share/doc/foo/copyright", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/share/doc/foo/license.el", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "usr/share/common-licenses/GPL-3",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo/license",
                TestFile::new("x").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: false,
                }),
            )]),
            // lintian's `is_executable` is any execute bit, so a directory with
            // mode 0755 is skipped too.
            BinaryPackageCase::from(vec![(
                "usr/share/perl5/Regexp/Pattern/License",
                TestFile::directory().mode(0o755),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/share/pixmaps/COPYING", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/share/pixmaps/license.foo", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/share/pixmaps/license.txt", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/files-extra-license/license.txt",
                TestFile::new("x"),
            )]),
        ];
        BinaryPackageTester::new(ExtraLicenseFile, pass, fail).test_and_snapshot();
    }
}
