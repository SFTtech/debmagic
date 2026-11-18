use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::interpreter::{Hashbang, basename};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch_triplets;

pub struct OldStyleConfigScriptMultiarchPath;

declare_rule! {
    /// An old style config script in `usr/bin/` names an architecture-specific
    /// directory, and the Binary package is Multi-Arch and not `all`.
    ///
    /// Walks Data files that are regular files (not hardlinks) named
    /// `usr/bin/*-config` that start with `#!` (except `#!#!#!`). Only the
    /// first 8192 bytes are read, as lintian's sliding window does, and the
    /// script must contain `flag`, `/include/` or `pkg-config` there. Only
    /// when Multi-Arch is set and not `no`, and Architecture is not `all`,
    /// does a file count. The first `DEB_HOST_MULTIARCH` triplet, in sorted
    /// order, that appears as a whole word in the window is reported unless
    /// the file name starts with that triplet. Extra is `full text contains
    /// architecture specific dir <triplet>`; the Data file is the Location.
    OldStyleConfigScriptMultiarchPath,
    code = "LN0734",
    tag = "old-style-config-script-multiarch-path",
    default_selected = true,
    experimental = false,
}

/// Bytes read for the first sliding window of a file.
const WINDOW_BYTES: usize = 8192;

impl BinaryPackageRule for OldStyleConfigScriptMultiarchPath {
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
        if multi_arch.is_empty() || multi_arch == "no" || architecture == "all" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let triplets = deb_host_multiarch_triplets();
        let hits: Vec<(PathBuf, &'static str)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| is_config_name(&entry.path().to_string_lossy()))
            .filter_map(|entry| {
                let bytes = files.read(entry).ok()?;
                Hashbang::read(&bytes)?;
                let window = &bytes[..bytes.len().min(WINDOW_BYTES)];
                if !is_config_text(window) {
                    return None;
                }
                let name = basename(&entry.path().to_string_lossy()).to_string();
                triplets
                    .iter()
                    .find(|triplet| {
                        contains_triplet(window, triplet) && !name.starts_with(**triplet)
                    })
                    .map(|triplet| (entry.path().to_path_buf(), *triplet))
            })
            .collect();
        for (path, triplet) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!(
                    "full text contains architecture specific dir {triplet}"
                ))
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// `usr/bin/` prefix and `-config` suffix.
fn is_config_name(name: &str) -> bool {
    name.starts_with("usr/bin/") && name.ends_with("-config")
}

/// lintian's old style config markers, matched as plain substrings.
fn is_config_text(window: &[u8]) -> bool {
    contains(window, b"flag") || contains(window, b"/include/") || contains(window, b"pkg-config")
}

/// `\W<triplet>(\W|$)`: the triplet with a non-word byte on both sides, or
/// the end of the window after it. A triplet at the very start does not match.
fn contains_triplet(window: &[u8], triplet: &str) -> bool {
    let needle = triplet.as_bytes();
    (1..window.len()).any(|start| {
        window[start..].starts_with(needle)
            && !is_word(window[start - 1])
            && window
                .get(start + needle.len())
                .is_none_or(|&byte| !is_word(byte))
    })
}

fn contains(window: &[u8], needle: &[u8]) -> bool {
    window
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(text: &str) -> TestFile {
        TestFile::new(format!("#!/bin/sh\necho \"-I/usr/include/{text}/foo.h\"\n"))
    }

    fn foreign_amd64(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Multi-Arch", "foreign")
    }

    #[test]
    fn triplet_boundaries() {
        let check = |text: &str| contains_triplet(text.as_bytes(), "x86_64-linux-gnu");
        assert!(check("-I/usr/include/x86_64-linux-gnu/foo"));
        assert!(check("\nx86_64-linux-gnu\n"));
        // Nothing precedes the triplet at the start of the window.
        assert!(!check("x86_64-linux-gnu"));
        assert!(!check("x86_64-linux-gnu_foo"));
        assert!(!check("ax86_64-linux-gnu"));
        assert!(!check("x86_64-linux-gnux32"));
        assert!(!check("x86_64-linux-gn"));
    }

    #[test]
    fn config_text_markers() {
        assert!(is_config_text(b"flag"));
        assert!(is_config_text(b"echo -I/include/x"));
        assert!(is_config_text(b"pkg-config --libs"));
        assert!(!is_config_text(b"/include"));
        assert!(!is_config_text(b"Flag"));
    }

    #[test]
    fn test() {
        let pass = vec![
            // Multi-Arch no and an absent Multi-Arch are not judged.
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "no"),
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))])
                .field("Architecture", "amd64"),
            // Architecture all belongs to the arch-all tag.
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))])
                .field("Architecture", "all")
                .field("Multi-Arch", "foreign"),
            // The file name starts with the triplet it names.
            foreign_amd64(vec![(
                "usr/bin/x86_64-linux-gnu_-foo-config",
                script("x86_64-linux-gnu"),
            )]),
            // No config markers in the first window.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new("#!/bin/sh\necho x86_64-linux-gnu\n"),
            )]),
            // Not under usr/bin, not a -config name, not a script.
            foreign_amd64(vec![("usr/lib/foo-config", script("x86_64-linux-gnu"))]),
            foreign_amd64(vec![("usr/bin/foo", script("x86_64-linux-gnu"))]),
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new("echo /include/x86_64-linux-gnu\n"),
            )]),
            // A triplet glued to a word character is not a match.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new("#!/bin/sh\n/include/x86_64-linux-gnu_foo\n"),
            )]),
            // A symlink, and a hardlink that is not the preferred name, are not regular files.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::symlink("bar-config"),
            )]),
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                script("x86_64-linux-gnu").hardlink(),
            )]),
            // The triplet lies beyond the first 8192 bytes.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new(format!(
                    "#!/bin/sh\n{}\n/include/x86_64-linux-gnu\n",
                    " ".repeat(WINDOW_BYTES)
                )),
            )]),
            // The config marker lies beyond the first 8192 bytes.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new(format!(
                    "#!/bin/sh\nx86_64-linux-gnu\n{}/include/\n",
                    " ".repeat(WINDOW_BYTES)
                )),
            )]),
        ];
        let fail = vec![
            foreign_amd64(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))]),
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("x86_64-linux-gnu"))])
                .field("Architecture", "any")
                .field("Multi-Arch", "allowed"),
            // The triplet is a different architecture than the file name.
            foreign_amd64(vec![(
                "usr/bin/x86_64-linux-gnu_-cross-config",
                script("i386-linux-gnu"),
            )]),
            // The trigger and the triplet are inside the window, the tail is ignored.
            foreign_amd64(vec![(
                "usr/bin/foo-config",
                TestFile::new(format!(
                    "#!/bin/sh\n/include/\nx86_64-linux-gnu\n{}\n",
                    " ".repeat(WINDOW_BYTES)
                )),
            )]),
            foreign_amd64(vec![
                ("usr/bin/a-config", script("x86_64-linux-gnu")),
                ("usr/bin/b-config", script("x86_64-linux-gnu")),
            ]),
        ];
        BinaryPackageTester::new(OldStyleConfigScriptMultiarchPath, pass, fail).test_and_snapshot();
    }
}
