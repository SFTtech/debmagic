use std::collections::HashSet;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::declare_rule;
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
        for member in ar_members(&bytes) {
            provided.insert(dir.join(member));
        }
    }
    provided
}

/// The member names of an `ar` archive, as `ar t` prints them. Empty when
/// the bytes are not an archive; a truncated header ends the list.
fn ar_members(bytes: &[u8]) -> Vec<String> {
    const MAGIC: &[u8] = b"!<arch>\n";
    let mut members = Vec::new();
    if !bytes.starts_with(MAGIC) {
        return members;
    }
    let mut long_names: &[u8] = b"";
    let mut position = MAGIC.len();
    while position + 60 <= bytes.len() {
        let header = &bytes[position..position + 60];
        let raw_name = &header[..16];
        let size: usize = match std::str::from_utf8(&header[48..58])
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            Some(size) => size,
            None => break,
        };
        let data_start = position + 60;
        let data_end = data_start.saturating_add(size).min(bytes.len());
        let data = &bytes[data_start..data_end];

        if raw_name.starts_with(b"//") {
            // The long-name table.
            long_names = data;
        } else if raw_name.starts_with(b"/") && raw_name[1..].iter().all(u8::is_ascii_digit) {
            // A GNU long name: an offset into the table, ended by `/\n`.
            let offset: usize = std::str::from_utf8(&raw_name[1..])
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(usize::MAX);
            let table = long_names.get(offset..).unwrap_or(&[]);
            let end = table
                .windows(2)
                .position(|pair| pair == b"/\n")
                .unwrap_or(table.len());
            members.push(String::from_utf8_lossy(&table[..end]).into_owned());
        } else if raw_name.starts_with(b"#1/") {
            // A BSD long name: the first bytes of the data.
            let length: usize = std::str::from_utf8(&raw_name[3..])
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(0);
            let name = &data[..length.min(data.len())];
            members.push(String::from_utf8_lossy(name).into_owned());
        } else if raw_name.starts_with(b"/ ") {
            // The archive symbol table, which `ar t` does not list.
        } else {
            let name = raw_name
                .split(|&byte| byte == b'/')
                .next()
                .unwrap_or_default();
            members.push(String::from_utf8_lossy(name).trim_end().to_string());
        }
        position = data_start + size + (size % 2);
    }
    members
}

#[cfg(test)]
mod tests {
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
        assert_eq!(ar_members(&archive_bytes(&["a.o", "b.o"])), ["a.o", "b.o"]);
        assert!(ar_members(b"not an archive").is_empty());
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
