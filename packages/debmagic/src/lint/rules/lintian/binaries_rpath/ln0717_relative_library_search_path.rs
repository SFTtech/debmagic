use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::rule::BinaryPackageRule;

pub struct RelativeLibrarySearchPath;

declare_rule! {
    /// The Binary package installs an ELF file whose RPATH or RUNPATH is
    /// relative.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks). For
    /// each `DT_RPATH` then `DT_RUNPATH` entry (split on `:`), a duplicate
    /// within one section is reported once. Entries that are empty, absolute
    /// (`custom-library-search-path`), or start with `$ORIGIN` or `${ORIGIN`
    /// are not hits. Hits are canonicalised like `File::Spec->canonpath`:
    /// repeated and `.` components and a trailing slash are dropped. Files
    /// under `usr/lib/haskell-packages/ghc` are skipped. Extra is the section
    /// name and the canonical entry (`RUNPATH usr/lib`); the path is the
    /// Location. Lintian's private-folder and `usr/lib/ghc` exclusions only
    /// apply to absolute entries, so they do not change this Rule.
    RelativeLibrarySearchPath,
    code = "LN0717",
    tag = "relative-library-search-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RelativeLibrarySearchPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, extra) in relative_entries(&elf_files) {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn relative_entries(elf_files: &ElfFiles) -> Vec<(PathBuf, String)> {
    let mut hits = Vec::new();
    for (path, facts) in elf_files.iter() {
        if path.starts_with(GHC_PACKAGES) {
            continue;
        }
        for (section, entries) in [("RPATH", facts.rpath()), ("RUNPATH", facts.runpath())] {
            let mut seen = BTreeSet::new();
            for entry in entries.iter().filter(|entry| seen.insert(entry.as_str())) {
                if is_relative(entry) {
                    hits.push((
                        path.to_path_buf(),
                        format!("{section} {}", canonical(entry)),
                    ));
                }
            }
        }
    }
    hits
}

fn is_relative(entry: &str) -> bool {
    !entry.is_empty()
        && !entry.starts_with('/')
        && !entry.starts_with("$ORIGIN")
        && !entry.starts_with("${ORIGIN")
}

/// `File::Spec->canonpath` for a relative path: no empty or `.` components,
/// no trailing slash. A path made only of `.` components is `.`.
fn canonical(entry: &str) -> String {
    let parts: Vec<&str> = entry
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

const GHC_PACKAGES: &str = "usr/lib/haskell-packages/ghc";

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn with_rpath(path: &str) -> TestFile {
        elf(TestElf::executable().needed("libc.so.6").rpath(path))
    }

    fn with_runpath(path: &str) -> TestFile {
        elf(TestElf::shared_object().soname("libfoo.so.1").runpath(path))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/bin/plain",
                elf(TestElf::executable().needed("libc.so.6")),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/absolute", with_rpath("/usr/lib/foo"))]),
            BinaryPackageCase::from(vec![("usr/bin/origin", with_rpath("$ORIGIN/../lib"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/origin-braces",
                with_runpath("${ORIGIN}/../lib"),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/empty", with_rpath(""))]),
            BinaryPackageCase::from(vec![("usr/bin/colons", with_rpath(":/usr/lib:"))]),
            BinaryPackageCase::from(vec![("usr/bin/script", TestFile::new("usr/lib\n"))]),
            BinaryPackageCase::from(vec![("usr/bin/link", TestFile::symlink("plain"))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/haskell-packages/ghc/lib/libfoo.so",
                with_runpath("lib"),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/foo.o", elf(TestElf::relocatable()))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/rpath", with_rpath("usr/lib"))]),
            BinaryPackageCase::from(vec![("usr/bin/runpath", with_runpath("usr/share/foo"))]),
            BinaryPackageCase::from(vec![("usr/bin/dot", with_rpath("./usr//lib/"))]),
            BinaryPackageCase::from(vec![("usr/bin/dot-only", with_rpath("./"))]),
            BinaryPackageCase::from(vec![("usr/bin/dup", with_rpath("lib:lib/"))]),
            BinaryPackageCase::from(vec![("usr/bin/mixed", with_rpath("/usr/lib:lib"))]),
            BinaryPackageCase::from(vec![("usr/bin/quoted", with_runpath("\"/usr/share/foo\""))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/both",
                elf(TestElf::executable()
                    .rpath("usr/lib")
                    .runpath("usr/share/foo")),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/hardlink", with_rpath("lib").hardlink())]),
            BinaryPackageCase::from(vec![(
                "usr/bin/udeb",
                elf(TestElf::executable().rpath("lib")),
            )])
            .udeb(),
            BinaryPackageCase::from(vec![(
                "usr/lib/haskell-packages/ghcfoo/lib/libfoo.so",
                with_runpath("lib"),
            )]),
        ];
        BinaryPackageTester::new(RelativeLibrarySearchPath, pass, fail).test_and_snapshot();
    }
}
