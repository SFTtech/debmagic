use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SourceContainsWafBinary;

declare_rule! {
    /// The Source tree or Source package ships a `waf` binary, not an unpacked one.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files) for regular files and hardlinks whose basename ends in `waf` at
    /// a word boundary (`waf`, `tools-waf`; not `mywaf`). Each is read, and a
    /// hit is a line starting with `#BZh` or `#BZ0` (a digit follows) that
    /// lies between a `#==>` line and the next `#<==` line: the embedded bzip2
    /// archive of a waf binary. Only the first hit per file is reported.
    /// Paths under `.pc/` are skipped, as lintian skips quilt's working copies.
    /// Symbolic links are never hits (lintian's `is_file` excludes them).
    /// Extra is empty; the file path is the Location with no line.
    SourceContainsWafBinary,
    code = "LN1011",
    tag = "source-contains-waf-binary",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceContainsWafBinary {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || entry.path().starts_with(".pc") || !basename_is_waf(entry.path())
            {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if contains_waf_archive(&bytes) {
                hits.push(entry.path().to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Lintian's `\b waf $` on the basename, with `\b` read as ASCII word bytes.
fn basename_is_waf(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let Some(name) = path.file_name() else {
        return false;
    };
    let name = name.as_bytes();
    let Some(stem) = name.strip_suffix(b"waf") else {
        return false;
    };
    stem.last().is_none_or(|byte| !is_word_byte(*byte))
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The embedded archive marker lintian looks for between `#==>` and `#<==`.
fn contains_waf_archive(bytes: &[u8]) -> bool {
    let mut inside = false;
    for line in bytes.split(|byte| *byte == b'\n') {
        if !line.starts_with(b"#") {
            continue;
        }
        if inside && is_bzip2_marker(line) {
            return true;
        }
        if line.starts_with(b"#==>") {
            inside = true;
        }
        if line.starts_with(b"#<==") {
            inside = false;
        }
    }
    false
}

/// `^#BZ[h0][0-9]`.
fn is_bzip2_marker(line: &[u8]) -> bool {
    matches!(line, [b'#', b'B', b'Z', b'h' | b'0', digit, ..] if digit.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const WAF_BINARY: &str = "#!/usr/bin/env python\n\
         #==>\n\
         #BZh91AY&SY\n\
         #<==\n";

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from(CONTROL))];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            // Only a basename ending in `waf` at a word boundary counts.
            tree(&[("mywaf", TestFile::new(WAF_BINARY))]),
            tree(&[("waf.in", TestFile::new(WAF_BINARY))]),
            tree(&[("waf.py", TestFile::new(WAF_BINARY))]),
            tree(&[("wafer", TestFile::new(WAF_BINARY))]),
            // An unpacked waf has no archive between the markers.
            tree(&[("waf", TestFile::new("#!/usr/bin/env python\nprint(1)\n"))]),
            // The marker must sit between `#==>` and `#<==`.
            tree(&[("waf", TestFile::new("#BZh91AY&SY\n"))]),
            tree(&[("waf", TestFile::new("#==>\n#<==\n#BZh91AY&SY\n"))]),
            // Only `#` lines count, and the marker needs a digit after `h` or `0`.
            tree(&[(
                "waf",
                TestFile::new("#==>\n BZh91AY&SY\n#==>\n#BZhx\n#<==\n"),
            )]),
            // Directories are not files.
            tree(&[("waf", TestFile::directory())]),
            // Symbolic links are not files.
            tree(&[("tools/waf", TestFile::symlink("../waf-real"))]),
            // Quilt's working copies are skipped.
            tree(&[(".pc/unpacked/waf", TestFile::new(WAF_BINARY))]),
        ];
        let fail = vec![
            tree(&[("waf", TestFile::new(WAF_BINARY))]),
            tree(&[("tools/waf", TestFile::new(WAF_BINARY))]),
            tree(&[("tools-waf", TestFile::new(WAF_BINARY))]),
            tree(&[("waf", TestFile::new("#==>\n#BZ07\n#<==\n"))]),
            // A hardlink is a file.
            tree(&[("waf", TestFile::new(WAF_BINARY).hardlink())]),
            // Several markers in one file still give one hit.
            tree(&[(
                "waf",
                TestFile::new("#==>\n#BZh9\n#<==\n#==>\n#BZh9\n#<==\n"),
            )]),
        ];
        SourceTreeTester::new(SourceContainsWafBinary, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", TestFile::from(CONTROL))]),
            SourcePackageCase::from(vec![("waf.in", TestFile::new(WAF_BINARY))]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("waf", TestFile::new(WAF_BINARY))]),
            SourcePackageCase::from(vec![("tools/waf", TestFile::new(WAF_BINARY))]),
        ];
        SourcePackageTester::new(SourceContainsWafBinary, pass, fail).test_and_snapshot();
    }
}
