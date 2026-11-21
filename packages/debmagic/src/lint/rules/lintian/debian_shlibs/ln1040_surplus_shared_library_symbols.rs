use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct SurplusSharedLibrarySymbols;

declare_rule! {
    /// The `symbols` control file declares a shared library that the Binary
    /// package does not install.
    ///
    /// Lintian's warning, one Diagnostic per distinct declared library, with
    /// the human name (`libfoo 1` for `libfoo.so.1`, `libfoo-1.so` too) as
    /// Extra and `symbols` as the Location (no line). A library is declared by
    /// a header line of the control file: a non-blank line that does not start
    /// with whitespace, `#`, `|` or `*`, and whose first field is the SONAME.
    /// A declared library is used when some installed ELF file (regular file or
    /// hardlink) has that SONAME, compared by human name. A symbols file that
    /// is not UTF-8 declares nothing, as in lintian. Applies to deb, udeb, and
    /// ddeb.
    SurplusSharedLibrarySymbols,
    code = "LN1040",
    tag = "surplus-shared-library-symbols",
    default_selected = true,
    experimental = false,
}

/// Lintian's soname header of a symbols file: the SONAME, then the main
/// dependency template.
static SONAME_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname header")
});
static LIB_SO_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)\.so\.(.*)$").expect("libfoo.so.X.X"));
static LIB_DASH_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)-(\d.*)\.so$").expect("libfoo-X.X.so"));

impl BinaryPackageRule for SurplusSharedLibrarySymbols {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(declared) = declared_sonames(ctx) else {
            return;
        };
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let used: HashSet<String> = installed_sonames(&elf_files)
            .iter()
            .map(|soname| human_soname(soname))
            .collect();
        let surplus: BTreeSet<String> = declared
            .iter()
            .map(|soname| human_soname(soname))
            .filter(|pretty| !used.contains(pretty))
            .collect();
        for pretty in surplus {
            ctx.diagnostic(Diagnostic::warn(pretty).with_location(Location {
                path: "symbols".into(),
                line: None,
                column: None,
            }));
        }
    }
}

/// SONAMEs of the `symbols` control file's header lines, in file order and
/// without duplicates. `None` when there is no symbols file.
fn declared_sonames(ctx: &mut BinaryPackageContext<'_>) -> Option<Vec<String>> {
    let files = ctx.control_files()?;
    let entry = files.get("symbols")?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let text = files
        .read(entry)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    let mut sonames: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(captures) = SONAME_HEADER.captures(line) {
            let soname = captures[1].to_string();
            if !sonames.contains(&soname) {
                sonames.push(soname);
            }
        }
    }
    Some(sonames)
}

/// The SONAME of every installed ELF file that has one.
fn installed_sonames(elf_files: &ElfFiles) -> Vec<String> {
    elf_files
        .iter()
        .filter_map(|(_, facts)| facts.soname().first().cloned())
        .collect()
}

/// Lintian's `human_soname`: `libfoo.so.X.X` and `libfoo-X.X.so` become
/// `libfoo X.X`; anything else is returned unchanged.
fn human_soname(soname: &str) -> String {
    if let Some(captures) = LIB_SO_VERSION
        .captures(soname)
        .or_else(|| LIB_DASH_VERSION.captures(soname))
    {
        return format!("{} {}", &captures[1], &captures[2]);
    }
    soname.to_string()
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(soname: &str) -> TestFile {
        TestFile::from_bytes(TestElf::shared_object().soname(soname).build())
    }

    fn case(symbols: &str, libraries: &[(&str, &str)]) -> BinaryPackageCase {
        let files: Vec<(String, TestFile)> = libraries
            .iter()
            .map(|(path, soname)| (path.to_string(), elf(soname)))
            .collect();
        BinaryPackageCase::from(files).control_file("symbols", symbols)
    }

    const HELLO: &str = "libhello.so.0 libhello0 #MINVER#\n hello@Base 1.0\n";

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case(HELLO, &[("usr/lib/libhello.so.0", "libhello.so.0")]).udeb(),
            case(HELLO, &[("usr/lib/libhello.so.0", "libhello.so.0")]),
            case(
                HELLO,
                &[
                    ("usr/lib/libhello.so.0", "libhello.so.0"),
                    ("usr/lib/libother.so.1", "libother.so.1"),
                ],
            ),
            // Same human name, different layout.
            case(
                "libfoo-1.so libfoo1 1.0\n a 1\n",
                &[("usr/lib/libfoo.so.1", "libfoo.so.1")],
            ),
            case(
                "libfoo.so.1 libfoo1 1.0\n",
                &[("usr/lib/libfoo-1.so", "libfoo-1.so")],
            ),
            // Comments and blank lines declare nothing.
            case("# libhello.so.0\n\n", &[]),
            // Indented symbol lines are not headers.
            case(" libhello.so.0 x\n", &[]),
            // A file that is not UTF-8 declares nothing.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "symbols",
                TestFile::from_bytes(b"libhello.so.0 libhello0 1.0\n\xff\n".to_vec()),
            ),
            // A library without SONAME is not used by any declaration.
            BinaryPackageCase::from(vec![(
                "usr/lib/libhello.so.0".to_string(),
                TestFile::from_bytes(TestElf::shared_object().build()),
            )])
            .control_file("symbols", "| libhello.so.0 libhello0 1.0\n"),
        ];
        let fail = vec![
            case(HELLO, &[]),
            case(HELLO, &[("usr/lib/libhello.so.1", "libhello.so.1")]),
            case(
                "libhello.so.0 libhello0 1.0\nlibhello.so.2 libhello2 2.0\n",
                &[("usr/lib/libhello.so.2", "libhello.so.2")],
            ),
            case(HELLO, &[]).udeb(),
            case("libhello.so.0 libhello0 1.0\n", &[]).package("hello"),
            case(
                "libhello.so.0 libhello0 1.0\nlibhello.so.0 again 1.0\n",
                &[("usr/lib/libhello.so.1", "libhello.so.1")],
            ),
        ];
        BinaryPackageTester::new(SurplusSharedLibrarySymbols, pass, fail).test_and_snapshot();
    }
}
