use std::collections::HashSet;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::ar;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct OcamlDanglingCmx;

declare_rule! {
    /// A `.cmx` OCaml byte-code module with no matching `.o` object: neither a
    /// `<stem>.o` file in its directory nor a member of that name in a `.a`
    /// archive of the same directory. Reported with an empty extra and the
    /// `.cmx` path as the Location. Applies to every entry of the Data files
    /// whose name ends in `.cmx`, as lintian's check does.
    ///
    /// The archive members come from the `.a` regular files of the Data files
    /// that are archives (`!<arch>` magic), read by a small private parser that
    /// follows the GNU long-name table and the BSD `#1/` names. Lintian lists
    /// them with `ar t`, which the Rule does not run.
    OcamlDanglingCmx,
    code = "LN1399",
    tag = "ocaml-dangling-cmx",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OcamlDanglingCmx {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let provided = provided_objects(files);
        let mut paths = Vec::new();
        for entry in files.entries() {
            let name = entry.path().as_os_str().as_bytes();
            if !name.ends_with(b".cmx") {
                continue;
            }
            let Some(stem) = object_stem(entry.path()) else {
                continue;
            };
            let object = entry
                .path()
                .parent()
                .unwrap_or(Path::new(""))
                .join(format!("{stem}.o"));
            if files.get(&object).is_none() && !provided.contains(&object) {
                paths.push(entry.path().to_path_buf());
            }
        }
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// `<stem>` of `<stem>.cmx`: the file name without its last extension.
fn object_stem(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let (stem, _) = name.rsplit_once('.')?;
    Some(stem.to_string())
}

/// The `<dir>/<member>` paths of every object in an archive of the Data files.
fn provided_objects(files: &FileTree) -> HashSet<PathBuf> {
    let mut provided = HashSet::new();
    for entry in files.entries() {
        let is_archive =
            entry.is_regular_file() && entry.path().as_os_str().as_bytes().ends_with(b".a");
        if !is_archive {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let dir = entry.path().parent().unwrap_or(Path::new(""));
        for member in ar::members(&bytes) {
            provided.insert(dir.join(member.name));
        }
    }
    provided
}

#[cfg(test)]
mod tests {
    use crate::lint::ar;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// The bytes of an `ar` archive holding `members`, each with content `x`.
    fn archive_bytes(members: &[&str]) -> Vec<u8> {
        let mut bytes = b"!<arch>\n".to_vec();
        for member in members {
            bytes.extend(
                format!(
                    "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                    format!("{member}/"),
                    0,
                    0,
                    0,
                    644,
                    2
                )
                .as_bytes(),
            );
            bytes.extend(b"x\n");
        }
        bytes
    }

    fn archive(members: &[&str]) -> TestFile {
        TestFile::from_bytes(archive_bytes(members))
    }

    fn case(files: Vec<(&'static str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    #[test]
    fn archive_members() {
        let bytes = archive_bytes(&["a.o", "b.o"]);
        let names: Vec<String> = ar::members(&bytes).into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["a.o", "b.o"]);
        assert!(ar::members(b"not an archive").is_empty());
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![
                ("usr/lib/ocaml/symbol.cmx", TestFile::new("x")),
                ("usr/lib/ocaml/symbol.o", TestFile::new("x")),
            ]),
            // The object is a member of an archive of the same directory.
            case(vec![
                ("usr/lib/ocaml/symbol.cmx", TestFile::new("x")),
                ("usr/lib/ocaml/libsym.a", archive(&["symbol.o"])),
            ]),
            // Not a `.cmx` name.
            case(vec![("usr/lib/ocaml/symbol.cmxa", TestFile::new("x"))]),
        ];
        let fail = vec![
            // A member in an archive of another directory does not count.
            case(vec![
                ("usr/lib/ocaml/symbol.cmx", TestFile::new("x")),
                ("usr/lib/other/libsym.a", archive(&["symbol.o"])),
            ]),
            case(vec![("usr/lib/ocaml/symbol.cmx", TestFile::new("x"))]),
            // The archive lacks the object.
            case(vec![
                ("usr/lib/ocaml/symbol.cmx", TestFile::new("x")),
                ("usr/lib/ocaml/libsym.a", archive(&["other.o"])),
            ]),
            // An archive that is not an archive provides nothing.
            case(vec![
                ("usr/lib/ocaml/symbol.cmx", TestFile::new("x")),
                ("usr/lib/ocaml/libsym.a", TestFile::new("symbol.o")),
            ]),
        ];
        BinaryPackageTester::new(OcamlDanglingCmx, pass, fail).test_and_snapshot();
    }
}
