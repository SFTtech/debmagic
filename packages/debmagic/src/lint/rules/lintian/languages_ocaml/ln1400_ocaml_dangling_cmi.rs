use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OcamlDanglingCmi;

declare_rule! {
    /// A `*.cmi` OCaml byte code interface is installed without its text
    /// version, a `*.mli` or `*.ml` file of the same module.
    ///
    /// The module name is the file name without its last extension, reduced
    /// to its last `__` part (dune). A sibling in the same directory is looked
    /// up as `<module>.mli`, `<lowercase module>.mli`, `<module>.ml`, then
    /// `<lowercase module>.ml`; any kind of entry counts. Reported at the
    /// `.cmi` path with an empty extra. Udebs are checked too.
    OcamlDanglingCmi,
    code = "LN1400",
    tag = "ocaml-dangling-cmi",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OcamlDanglingCmi {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.to_str().is_some_and(|name| name.ends_with(".cmi")))
            .filter(|path| {
                let candidates = source_candidates(path);
                candidates
                    .iter()
                    .all(|candidate| files.get(candidate).is_none())
            })
            .map(Path::to_path_buf)
            .collect();
        for path in paths {
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

/// The `.mli` and `.ml` siblings of `path` that could hold its text version.
fn source_candidates(path: &Path) -> Vec<PathBuf> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    // Perl's `split(/__/, ...)` drops trailing empty fields, then `[-1]`.
    let mut parts: Vec<&str> = stem.split("__").collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    let module = parts.last().copied().unwrap_or("");
    let lower = module.to_lowercase();
    let mut names = Vec::new();
    for base in [module.to_string(), lower] {
        for extension in ["mli", "ml"] {
            names.push(format!("{base}.{extension}"));
        }
    }
    names.iter().map(|name| path.with_file_name(name)).collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(vec![
                ("usr/lib/ocaml/foo.cmi", TestFile::new("x")),
                ("usr/lib/ocaml/foo.mli", TestFile::new("x")),
            ]),
            tree(vec![
                ("usr/lib/ocaml/foo.cmi", TestFile::new("x")),
                ("usr/lib/ocaml/foo.ml", TestFile::new("x")),
            ]),
            // Lowercase sibling of a capitalised module.
            tree(vec![
                ("usr/lib/ocaml/Foo.cmi", TestFile::new("x")),
                ("usr/lib/ocaml/foo.mli", TestFile::new("x")),
            ]),
            // dune: the last `__` part names the module.
            tree(vec![
                ("usr/lib/ocaml/lib__foo.cmi", TestFile::new("x")),
                ("usr/lib/ocaml/foo.ml", TestFile::new("x")),
            ]),
            // Any kind of entry is the sibling.
            tree(vec![
                ("usr/lib/ocaml/foo.cmi", TestFile::new("x")),
                ("usr/lib/ocaml/foo.mli", TestFile::directory()),
            ]),
            // Only the `.cmi` extension is checked.
            tree(vec![("usr/lib/ocaml/foo.cmi.txt", TestFile::new("x"))]),
            tree(vec![("usr/lib/ocaml/foo.cmti", TestFile::new("x"))]),
        ];
        let fail = vec![
            tree(vec![("usr/lib/ocaml/foo.cmi", TestFile::new("x"))]),
            // The sibling must be in the same directory.
            tree(vec![
                ("usr/lib/ocaml/foo.cmi", TestFile::new("x")),
                ("usr/lib/foo.mli", TestFile::new("x")),
            ]),
            // A `.cmi` without a stem has no `.mli`/`.ml` candidate.
            tree(vec![("usr/lib/ocaml/.cmi", TestFile::new("x"))]),
        ];
        BinaryPackageTester::new(OcamlDanglingCmi, pass, fail).test_and_snapshot();
    }
}
