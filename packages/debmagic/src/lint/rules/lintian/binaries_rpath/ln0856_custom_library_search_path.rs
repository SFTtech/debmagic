use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch;

pub struct CustomLibrarySearchPath;

declare_rule! {
    /// The Binary package installs an ELF file whose RPATH or RUNPATH is an
    /// absolute path outside the private library folders.
    ///
    /// Walks the ELF facts of Data files (regular files and hardlinks). For
    /// each `DT_RPATH` then `DT_RUNPATH` entry, a duplicate raw entry within
    /// one section is reported once. Entries that start with `$ORIGIN` or
    /// `${ORIGIN`, are empty, or are relative are not hits (relative entries
    /// are `relative-library-search-path`). Absolute entries are canonicalised
    /// like `File::Spec->canonpath` (repeated and `.` components and a trailing
    /// slash are dropped). An entry is not a hit when it starts with one of the
    /// private folders `/lib/`, `/usr/lib/`, `/usr/lib/games/` (and the
    /// `DEB_HOST_MULTIARCH` variants of `lib` and `usr/lib`) followed by the
    /// Source package name, or it is under `/usr/lib/ghc` or
    /// `/usr/lib/haskell-packages/ghc`. Files under
    /// `usr/lib/haskell-packages/ghc` are skipped. Extra is the section name
    /// and the canonical entry (`RUNPATH /usr/lib`); the path is the Location.
    CustomLibrarySearchPath,
    code = "LN0856",
    tag = "custom-library-search-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CustomLibrarySearchPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .unwrap_or_default()
            .trim()
            .to_string();
        let private = private_folders(&ctx.source_name(), deb_host_multiarch(&architecture));
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        for (path, extra) in custom_entries(&elf_files, &private) {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The `/`-prefixed private folders: `<folder>/<source>` for each library
/// folder, its multiarch variant, and the `games` subfolders of `usr/lib`.
fn private_folders(source: &str, multiarch: Option<&str>) -> Vec<String> {
    let mut lib = vec!["lib".to_string()];
    let mut usr_lib = vec!["usr/lib".to_string()];
    if let Some(triplet) = multiarch {
        lib.push(format!("lib/{triplet}"));
        usr_lib.push(format!("usr/lib/{triplet}"));
    }
    let games: Vec<String> = usr_lib.iter().map(|dir| format!("{dir}/games")).collect();
    lib.iter()
        .chain(usr_lib.iter())
        .chain(games.iter())
        .map(|dir| format!("/{dir}/{source}"))
        .collect()
}

fn custom_entries(elf_files: &ElfFiles, private: &[String]) -> Vec<(PathBuf, String)> {
    let mut hits = Vec::new();
    for (path, facts) in elf_files.iter() {
        if path.starts_with(GHC_PACKAGES) {
            continue;
        }
        for (section, entries) in [("RPATH", facts.rpath()), ("RUNPATH", facts.runpath())] {
            let mut seen = BTreeSet::new();
            for entry in entries.iter().filter(|entry| seen.insert(entry.as_str())) {
                if !entry.starts_with('/') {
                    continue;
                }
                let folder = canonical(entry);
                if private
                    .iter()
                    .any(|prefix| folder.starts_with(prefix.as_str()))
                {
                    continue;
                }
                if under(&folder, "/usr/lib/ghc") || under(&folder, "/usr/lib/haskell-packages/ghc")
                {
                    continue;
                }
                hits.push((path.to_path_buf(), format!("{section} {folder}")));
            }
        }
    }
    hits
}

/// `folder` is `dir` itself or lies below it.
fn under(folder: &str, dir: &str) -> bool {
    folder == dir || folder.starts_with(&format!("{dir}/"))
}

/// `File::Spec->canonpath` for an absolute path: no empty or `.` components,
/// no trailing slash.
fn canonical(entry: &str) -> String {
    let parts: Vec<&str> = entry
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    format!("/{}", parts.join("/"))
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
            // Relative entries belong to relative-library-search-path.
            BinaryPackageCase::from(vec![("usr/bin/relative", with_rpath("usr/lib"))]),
            BinaryPackageCase::from(vec![("usr/bin/origin", with_rpath("$ORIGIN/../lib"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/origin-braces",
                with_runpath("${ORIGIN}/../lib"),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/empty", with_rpath(""))]),
            BinaryPackageCase::from(vec![("usr/bin/script", TestFile::new("/usr/lib\n"))]),
            BinaryPackageCase::from(vec![("usr/bin/link", TestFile::symlink("plain"))]),
            // Private folders for the Source package.
            BinaryPackageCase::from(vec![("usr/bin/private", with_rpath("/usr/lib/foo"))])
                .field("Source", "foo"),
            BinaryPackageCase::from(vec![("usr/bin/private", with_runpath("/lib/foo/"))])
                .field("Source", "foo"),
            BinaryPackageCase::from(vec![(
                "usr/bin/private",
                with_runpath("/usr/lib/x86_64-linux-gnu/foo"),
            )])
            .field("Source", "foo")
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/bin/private",
                with_runpath("/usr/lib/games/foo"),
            )])
            .field("Source", "foo"),
            // GHC scheme.
            BinaryPackageCase::from(vec![("usr/bin/ghc", with_rpath("/usr/lib/ghc/lib"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/ghc",
                with_rpath("/usr/lib/haskell-packages/ghc/lib"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/haskell-packages/ghc/lib/libfoo.so",
                with_runpath("/usr/share/foo"),
            )]),
            // Not an ELF file.
            BinaryPackageCase::from(vec![("usr/bin/text", TestFile::new("/usr/lib\n"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/runpath", with_runpath("/usr/share/foo"))]),
            BinaryPackageCase::from(vec![("usr/bin/rpath", with_rpath("/usr/local/lib"))]),
            BinaryPackageCase::from(vec![("usr/bin/system", with_rpath("/usr/lib"))])
                .field("Source", "foo"),
            BinaryPackageCase::from(vec![("usr/bin/dot", with_rpath("/./usr//lib/"))]),
            BinaryPackageCase::from(vec![("usr/bin/dup", with_rpath("/usr/lib:/usr/lib/"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/both",
                elf(TestElf::executable()
                    .rpath("/usr/local/lib")
                    .runpath("/opt/foo")),
            )]),
            // Only a whole folder name counts for the GHC scheme.
            BinaryPackageCase::from(vec![("usr/bin/ghc2", with_rpath("/usr/lib/ghc2/lib"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/hardlink",
                with_rpath("/usr/local/lib").hardlink(),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("usr/bin/udeb", with_rpath("/usr/local/lib"))]).udeb(),
        ];
        BinaryPackageTester::new(CustomLibrarySearchPath, pass, fail).test_and_snapshot();
    }
}
