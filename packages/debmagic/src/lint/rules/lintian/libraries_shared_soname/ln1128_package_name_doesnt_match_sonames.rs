use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch;

pub struct PackageNameDoesntMatchSonames;

declare_rule! {
    /// The name of a library Binary package matches none of the SONAMEs of the
    /// shared objects it installs.
    ///
    /// Skipped: udebs; ELF files outside `lib`, `usr/lib`, and their
    /// `DEB_HOST_MULTIARCH` subdirectories; and `libnss_<name>.so.<digits>`
    /// modules. The SONAMEs of the rest are shortened as lintian does: a
    /// `.so.` after a digit becomes `-`, a `.so` before a `.` or at the end
    /// is dropped, and `_` becomes `-` (case kept for the extra). The
    /// package name, with transition suffixes stripped (`c102`, `c2`, `c2a`,
    /// `g`, `gf`, `v5`, `v6`, `t64`, `-udeb`, and a leading `lib64`
    /// read as `lib`), must match one SONAME, compared lowercased. When none
    /// matches, one warning lists the sorted SONAMEs (no Location).
    PackageNameDoesntMatchSonames,
    code = "LN1128",
    tag = "package-name-doesnt-match-sonames",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageNameDoesntMatchSonames {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let architecture = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .unwrap_or_default()
            .trim()
            .to_string();
        let component = deb_host_multiarch(&architecture);
        let mut common_folders: Vec<String> = vec!["lib".into(), "usr/lib".into()];
        if let Some(component) = component {
            common_folders.push(format!("lib/{component}"));
            common_folders.push(format!("usr/lib/{component}"));
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<std::path::PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path().to_path_buf())
            .collect();
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let mut duplicated: Vec<String> = Vec::new();
        for path in &paths {
            if !in_folder(path, &common_folders) || is_nss_module(path) {
                continue;
            }
            if let Some(facts) = elf_files.get(path) {
                duplicated.extend(facts.soname().iter().cloned());
            }
        }
        duplicated.sort();
        duplicated.dedup();
        if duplicated.is_empty() {
            return;
        }
        let shortened = shortened_name(&package);
        let transformed: Vec<String> = duplicated.iter().map(|soname| transform(soname)).collect();
        let matched = transformed.iter().any(|soname| {
            let lowercase = soname.to_lowercase();
            lowercase == package || lowercase == shortened
        });
        if matched {
            return;
        }
        let mut names = transformed;
        names.sort();
        ctx.diagnostic(Diagnostic::warn(names.join(" ")));
    }
}

/// `dirname` is exactly one of the folders, lintian's `$item->dirname eq $_ . '/'`.
fn in_folder(path: &Path, folders: &[String]) -> bool {
    path.parent()
        .is_some_and(|dir| folders.iter().any(|folder| Path::new(folder) == dir))
}

/// Lintian's `^libnss_[^.]+\.so(?:\.\d+)$` on the file name.
fn is_nss_module(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let Some(name) = path.file_name() else {
        return false;
    };
    let Some(rest) = name.as_bytes().strip_prefix(b"libnss_") else {
        return false;
    };
    let Some(dot) = rest.iter().position(|&byte| byte == b'.') else {
        return false;
    };
    let Some(digits) = rest[dot..].strip_prefix(b".so.") else {
        return false;
    };
    dot > 0 && !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

/// Lintian's transition-suffix stripping, in its order.
fn shortened_name(package: &str) -> String {
    let mut name = package.to_string();
    strip_after_digit(&mut name, "c102");
    strip_after_digit_one_of(&mut name, &["c2a", "c2"]);
    strip_after_digit(&mut name, "g");
    strip_after_digit(&mut name, "gf");
    strip_after_digit_one_of(&mut name, &["v5", "v6"]);
    strip_after_digit(&mut name, "t64");
    if let Some(stripped) = name.strip_suffix("-udeb") {
        name = stripped.to_string();
    }
    if let Some(rest) = name.strip_prefix("lib64") {
        name = format!("lib{rest}");
    }
    name
}

fn strip_after_digit(name: &mut String, suffix: &str) {
    strip_after_digit_one_of(name, &[suffix]);
}

/// `s/(\d)SUFFIX$/$1/`, for the first suffix that applies.
fn strip_after_digit_one_of(name: &mut String, suffixes: &[&str]) {
    for suffix in suffixes {
        if let Some(head) = name.strip_suffix(suffix)
            && head.ends_with(|c: char| c.is_ascii_digit())
        {
            name.truncate(head.len());
            return;
        }
    }
}

/// The SONAME as lintian prints it: the first `<digit>.so.` becomes
/// `<digit>-`, the first `.so` before a `.` or at the end is dropped, and
/// `_` becomes `-`.
fn transform(soname: &str) -> String {
    let mut name = soname.to_string();
    if let Some(at) = (0..name.len().saturating_sub(4))
        .find(|&at| name.as_bytes()[at].is_ascii_digit() && name[at + 1..].starts_with(".so."))
    {
        name.replace_range(at + 1..at + 5, "-");
    }
    let dropped = name.match_indices(".so").find_map(|(at, _)| {
        let after = &name[at + 3..];
        if after.starts_with('.') {
            Some((at, at + 4))
        } else if after.is_empty() {
            Some((at, at + 3))
        } else {
            None
        }
    });
    if let Some((start, end)) = dropped {
        name.replace_range(start..end, "");
    }
    name.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn library(soname: &str) -> TestFile {
        elf(TestElf::shared_object().soname(soname))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package("libfoo1"),
            // The package name matches the SONAME shortened (`libfoo1`).
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library("libfoo.so.1"))])
                .package("libfoo1"),
            BinaryPackageCase::from(vec![("usr/lib/libbaz2.so.1.0", library("libbaz2.so.1.0"))])
                .package("libbaz2-1.0"),
            // Transition suffix: `libfoo1c2` is read as `libfoo1`.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library("libfoo.so.1"))])
                .package("libfoo1c2"),
            // Case is ignored in the comparison.
            BinaryPackageCase::from(vec![("usr/lib/libFoo.so.1", library("libFoo.so.1"))])
                .package("libfoo1"),
            // Underscores read as dashes.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo_bar.so.1",
                library("libfoo_bar.so.1"),
            )])
            .package("libfoo-bar1"),
            // Outside the library directories.
            BinaryPackageCase::from(vec![(
                "usr/lib/apache2/modules/mod_x.so",
                library("mod_bogus.so"),
            )])
            .package("libfoo1"),
            // NSS modules are filtered out.
            BinaryPackageCase::from(vec![(
                "usr/lib/libnss_files.so.2",
                library("libnss_files.so.2"),
            )])
            .package("libfoo1"),
            // Udebs are not checked.
            BinaryPackageCase::from(vec![("usr/lib/libbogus.so.1", library("libbogus.so.1"))])
                .package("libfoo1")
                .udeb(),
            // Non-ELF file.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", TestFile::new("text"))])
                .package("libfoo1"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/libbogus.so.1", library("libbogus.so.1"))])
                .package("libfoo1"),
            // The hit lists every SONAME, shortened, sorted.
            BinaryPackageCase::from(vec![
                ("usr/lib/liba.so.2", library("libb.so.2")),
                ("usr/lib/libc.so", library("libbaz.so")),
            ])
            .package("libfoo1"),
            BinaryPackageCase::from(vec![(
                "lib/x86_64-linux-gnu/libz.so.1",
                library("libz.so.1"),
            )])
            .package("libfoo1")
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                library("libfoo.so.1").hardlink(),
            )])
            .package("libbogus1"),
        ];
        BinaryPackageTester::new(PackageNameDoesntMatchSonames, pass, fail).test_and_snapshot();
    }
}
