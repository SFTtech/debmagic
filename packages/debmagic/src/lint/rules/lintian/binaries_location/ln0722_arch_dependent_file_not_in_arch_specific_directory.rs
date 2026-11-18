use std::path::PathBuf;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch;

pub struct ArchDependentFileNotInArchSpecificDirectory;

declare_rule! {
    /// An ELF object or static archive in a Multi-Arch: same package is not in an architecture-specific directory.
    ///
    /// Walks Data files that are regular files or hardlinks and are ELF
    /// (any `ELF` file, as the ELF facts see it) or a static archive (`!<arch>`
    /// magic that is not a `.deb`). Only when the Binary package has
    /// `Multi-Arch: same` and its Architecture has a `DEB_HOST_MULTIARCH` triplet
    /// does a file count. It is a hit unless its path contains the triplet
    /// (followed by a word boundary or `_`), a `/<ruby triplet>/` directory
    /// (`linux-gnu` written as `linux`), a `/java-N-openjdk-<arch>/` directory
    /// (optionally `-jvmci`), or a `/.build-id/` directory. Extra is empty; the
    /// Data files path is the Location.
    ArchDependentFileNotInArchSpecificDirectory,
    code = "LN0722",
    tag = "arch-dependent-file-not-in-arch-specific-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ArchDependentFileNotInArchSpecificDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (multi_arch, architecture) = {
            let fields = ctx.installation_control().fields();
            (
                fields
                    .get("Multi-Arch")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                fields
                    .get("Architecture")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            )
        };
        if multi_arch != "same" {
            return;
        }
        let Some(triplet) = deb_host_multiarch(&architecture) else {
            return;
        };
        let patterns = Patterns::new(triplet, &architecture);

        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                elf_files.get(entry.path()).is_some()
                    || files
                        .read(entry)
                        .is_ok_and(|bytes| is_static_archive(&bytes))
            })
            .filter(|entry| !patterns.matches(&entry.path().to_string_lossy()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// `ar` archive with the GNU format magic, other than a `.deb` (whose first
/// member is `debian-binary`). `file(1)` calls these "current ar archive".
fn is_static_archive(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(b"!<arch>\n")
        .is_some_and(|members| !members.starts_with(b"debian-binary"))
}

/// The directory patterns that make a path architecture-specific, as
/// lintian's `binaries/location` builds them.
struct Patterns {
    gnu: Regex,
    ruby: Regex,
    java: Regex,
    build_id: &'static str,
}

impl Patterns {
    fn new(triplet: &str, architecture: &str) -> Self {
        let ruby = match triplet.strip_suffix("linux-gnu") {
            Some(prefix) => format!("{prefix}linux"),
            None => triplet.replacen("linux-gnu", "linux-", 1),
        };
        Self {
            gnu: Regex::new(&format!(r"\b{}(?:\b|_)", triplet_regex(triplet)))
                .expect("valid regex"),
            ruby: Regex::new(&format!("/{}/", triplet_regex(&ruby))).expect("valid regex"),
            java: Regex::new(&format!(
                r"/java-\d+-openjdk-{}(?:-jvmci)?/",
                regex::escape(architecture)
            ))
            .expect("valid regex"),
            build_id: "/.build-id/",
        }
    }

    fn matches(&self, name: &str) -> bool {
        self.gnu.is_match(name)
            || self.ruby.is_match(name)
            || self.java.is_match(name)
            || name.contains(self.build_id)
    }
}

/// Regex for a triplet; a leading `i386` also matches `i486` to `i686`.
fn triplet_regex(triplet: &str) -> String {
    let escaped = regex::escape(triplet);
    match escaped.strip_prefix("i386") {
        Some(rest) => format!("i[3-6]86{rest}"),
        None => escaped,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn archive() -> TestFile {
        TestFile::from_bytes(
            b"!<arch>\nlibfoo.o/      0           0     0     644     4         `\nabcd".to_vec(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Without Multi-Arch: same, the path does not matter.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "foreign"),
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "allowed"),
            // Architecture without a multiarch triplet.
            BinaryPackageCase::from(vec![("usr/bin/foo", elf(TestElf::executable()))])
                .field("Architecture", "all")
                .field("Multi-Arch", "same"),
            // Multiarch directories.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.a", archive())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/i386-linux-gnu/libfoo.so.1",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "i386")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/i686-linux-gnu/libfoo.so.1",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "i386")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu_blah/libfoo.so.1",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // Ruby, Java, and build-id directories.
            BinaryPackageCase::from(vec![(
                "usr/lib/ruby/3.0.0/x86_64-linux/foo.so",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/jvm/java-11-openjdk-amd64/lib/libjava.so",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/jvm/java-11-openjdk-amd64-jvmci/lib/libjvm.so",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/.build-id/ab/cdef.debug",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // Not an ELF object or a static archive.
            BinaryPackageCase::from(vec![("usr/bin/script", TestFile::new("#!/bin/sh\n"))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![("usr/share/doc/notes", TestFile::new("!<arch>text"))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/bin/link",
                TestFile::symlink("x86_64-linux-gnu/libfoo.so"),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // A .deb is not a static archive.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/bar.deb",
                TestFile::from_bytes(b"!<arch>\ndebian-binary   ".to_vec()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/same", elf(TestElf::executable()))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libbasic.so.2",
                elf(TestElf::shared_object().soname("libbasic.so.2")),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![("var/lib/private/same", elf(TestElf::executable()))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", archive())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // The triplet must end at a word boundary or `_`.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnufoo/libfoo.so.1",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // The Ruby triplet needs slashes around it.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux.so",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // Java directory of another architecture.
            BinaryPackageCase::from(vec![(
                "usr/lib/jvm/java-11-openjdk-arm64/lib/libjava.so",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/lib/build-id/ab/cdef.debug",
                elf(TestElf::shared_object()),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![(
                "usr/bin/same-link",
                elf(TestElf::executable()).hardlink_to("usr/bin/same-target"),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same"),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .field("Architecture", "amd64")
            .field("Multi-Arch", "same")
            .udeb(),
        ];
        BinaryPackageTester::new(ArchDependentFileNotInArchSpecificDirectory, pass, fail)
            .test_and_snapshot();
    }
}
