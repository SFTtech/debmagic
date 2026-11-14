use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, visit};

pub struct CompressedDocumentation;

declare_rule! {
    /// Two `compressed-documentation` conditions from the documentation Check.
    ///
    /// A path under `usr/share/doc/` whose index name matches
    /// `(?:doxygen|html)/` then `.map.` and a compressed extension from
    /// `files/compressed-file-extensions`. The extension is not end-anchored,
    /// so `foo.map.gzip` matches. Separately,
    /// `usr/share/doc/<package>/<dir>/…/objects.inv.gz` when the file begins
    /// with gzip magic `1f 8b` (`FileTree::read`). The package name is the
    /// Installation control `Package` field, case-sensitive. One path can
    /// emit both hints, map first. Skipped for udebs. Extra is empty.
    /// Directories keep a trailing slash.
    CompressedDocumentation,
    code = "LN0209",
    tag = "compressed-documentation",
    default_selected = true,
    experimental = false,
}

static MAP_COMPRESSED: LazyLock<Regex> = LazyLock::new(|| {
    let extensions = crate::lint::data::lines(include_str!(
        "../../../data/files/compressed-file-extensions"
    ))
    .map(regex::escape)
    .collect::<Vec<_>>()
    .join("|");
    Regex::new(&format!(
        "(?s)^usr/share/doc/(?:.+/)?(?:doxygen|html)/.*[.]map[.](?:{extensions})"
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for CompressedDocumentation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let objects = objects_pattern(ctx.package_name());
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for (entry, name) in visit(files) {
            if MAP_COMPRESSED.is_match(&name) {
                hits.push(name.clone());
            }
            if objects.is_match(&name) && is_gzip(files, entry) {
                hits.push(name);
            }
        }
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
    }
}

fn objects_pattern(package: &str) -> Regex {
    let package = regex::escape(package);
    Regex::new(&format!(
        r"^usr/share/doc/{package}/(?:[^/]+/)+objects[.]inv[.]gz$"
    ))
    .expect("valid regex")
}

/// Gzip magic from the file bytes. `FileType` is only ELF, PE, and MZ.
fn is_gzip(files: &FileTree, entry: &Entry) -> bool {
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip() -> TestFile {
        TestFile::from_bytes(vec![0x1f_u8, 0x8b])
    }

    fn files(paths: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let text = TestFile::new("not gzip");
        let pass = vec![
            files(&[
                ("usr/share/doc/Foo/docs/objects.inv.gz", text.clone()),
                ("usr/share/doc/Foo/objects.inv.gz", gzip()),
                (
                    "usr/share/doc/foo/docs/objects.inv.gz",
                    TestFile::from_bytes(vec![0x1f_u8, 0x8b]).file_type(FileType::Elf {
                        has_static_symbol_table: false,
                        executable: true,
                    }),
                ),
                ("usr/share/doc/html/foo.map.GZ", text.clone()),
                ("usr/share/doc/HTML/foo.map.gz", text.clone()),
                ("usr/share/doc/mydoxygen/foo.map.gz", text.clone()),
                ("usr/share/doc/html/foo.map", text),
                (
                    "usr/share/doc/example/docs/objects.inv.gz",
                    TestFile::directory(),
                ),
            ])
            .package("Foo"),
            files(&[("usr/share/doc/html/foo.map.gz", TestFile::new("x"))]).udeb(),
        ];
        let fail = vec![
            files(&[
                ("usr/share/doc/html/foo.map.gz", TestFile::new("plain")),
                ("usr/share/doc/doxygen/foo.map.bz2", TestFile::new("x")),
                ("usr/share/doc/html/foo.map.Z", TestFile::new("x")),
                ("usr/share/doc/html/foo.map.gzip", TestFile::new("x")),
                ("usr/share/doc/html/foo.map.gz.extra", TestFile::new("x")),
                (
                    "usr/share/doc/files-doxygen-documentation/html/doc/html/graph_legend.map.gz",
                    TestFile::new("x"),
                ),
                ("usr/share/doc/html/legend.map.xz", TestFile::directory()),
                (
                    "usr/share/doc/html/link.map.gz",
                    TestFile::symlink("elsewhere"),
                ),
            ]),
            files(&[(
                "usr/share/doc/python-kinterbasdb/docs/objects.inv.gz",
                gzip(),
            )])
            .package("python-kinterbasdb"),
            files(&[("usr/share/doc/example/html/x.map.gz/objects.inv.gz", gzip())]),
        ];
        BinaryPackageTester::new(CompressedDocumentation, pass, fail).test_and_snapshot();
    }
}
