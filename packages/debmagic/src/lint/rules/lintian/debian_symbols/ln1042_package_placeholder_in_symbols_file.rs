use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::SourceRule;

pub struct PackagePlaceholderInSymbolsFile;

declare_rule! {
    /// A `debian/*.symbols` file still contains the `#PACKAGE#` placeholder in
    /// its `* Build-Depends-Package:` meta-information field.
    ///
    /// Walks the regular files and hardlinks named `debian/<something>.symbols`
    /// (raw path bytes, at least one character before `.symbols`), skipping
    /// anything under `.pc/`. Each line is taken with its last byte dropped
    /// (lintian's `chop`), blank and `#` comment lines are skipped, and a line
    /// of the form `* <field>: <value>` whose field is exactly
    /// `Build-Depends-Package` and whose value contains `#PACKAGE#` is reported
    /// at that line. Line numbers count every line, skipped ones included.
    /// Extra is empty. Symlinks, fifos and devices are not inspected, as in
    /// lintian's `is_file` gate. A file whose content cannot be read is skipped.
    PackagePlaceholderInSymbolsFile,
    code = "LN1042",
    tag = "package-placeholder-in-symbols-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackagePlaceholderInSymbolsFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !is_symbols_file(entry) || !entry.is_file() {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            for line in placeholder_lines(&bytes) {
                hits.push((entry.path().to_path_buf(), line));
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Lintian's `visit_patched_files` name test: `debian/`, then at least one
/// byte, then `.symbols`. Paths under `.pc/` are dropped.
fn is_symbols_file(entry: &Entry) -> bool {
    let path = entry.path().as_os_str().as_bytes();
    if path.split(|&byte| byte == b'/').any(|part| part == b".pc") {
        return false;
    }
    let Some(rest) = path.strip_prefix(b"debian/") else {
        return false;
    };
    const SUFFIX: &[u8] = b".symbols";
    rest.len() > SUFFIX.len() && rest.ends_with(SUFFIX)
}

/// 1-based line numbers of the placeholder meta-information lines.
fn placeholder_lines(bytes: &[u8]) -> Vec<u32> {
    let mut lines = Vec::new();
    for (index, raw) in bytes.split_inclusive(|&byte| byte == b'\n').enumerate() {
        // Perl's `chop`: drop the trailing newline, or the last byte of a final
        // line that has none.
        let line = &raw[..raw.len() - 1];
        if is_blank_or_comment(line) {
            continue;
        }
        let Some((field, value)) = meta_field(line) else {
            continue;
        };
        if field == b"Build-Depends-Package" && contains(value, b"#PACKAGE#") {
            lines.push(u32::try_from(index + 1).unwrap_or(u32::MAX));
        }
    }
    lines
}

/// `^\s*$` or `^#`.
fn is_blank_or_comment(line: &[u8]) -> bool {
    line.first() == Some(&b'#') || line.iter().all(|&byte| is_perl_space(byte))
}

/// `^\*\s(\S+):\s+(\S+)`: the field (ending in the colon) and the value.
fn meta_field(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let rest = line.strip_prefix(b"*")?;
    let rest = rest.strip_prefix(&[*rest.first().filter(|&&b| is_perl_space(b))?])?;
    let run_end = rest
        .iter()
        .position(|&byte| is_perl_space(byte))
        .unwrap_or(rest.len());
    // The field is a non-space run that ends in the colon.
    let field = rest[..run_end].strip_suffix(b":")?;
    if field.is_empty() {
        return None;
    }
    let after = &rest[run_end..];
    let value_start = after.iter().position(|&byte| !is_perl_space(byte))?;
    if value_start == 0 {
        return None;
    }
    let value = &after[value_start..];
    let value_end = value
        .iter()
        .position(|&byte| is_perl_space(byte))
        .unwrap_or(value.len());
    if value_end == 0 {
        return None;
    }
    Some((field, &value[..value_end]))
}

/// Perl's `\s` on ASCII: space, tab, newline, carriage return, form feed, vertical tab.
fn is_perl_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const SYMBOLS: &str = "libfoo.so.1 libfoo1 #MINVER#\n\
         * Build-Depends-Package: libfoo-dev\n\
          foo_init@Base 1.0\n";

    const PLACEHOLDER: &str = "* Build-Depends-Package: #PACKAGE#\n";

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from("Source: x\n"))];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(&[]),
            tree(&[("debian/foo.symbols", TestFile::new(SYMBOLS))]),
            // Commented, other field, wrong name, wrong place, quilt copy.
            tree(&[(
                "debian/foo.symbols",
                TestFile::new(format!("# {PLACEHOLDER}")),
            )]),
            tree(&[(
                "debian/foo.symbols",
                TestFile::new("* Other-Field: #PACKAGE#\n"),
            )]),
            tree(&[("debian/.symbols", TestFile::new(PLACEHOLDER))]),
            tree(&[("debian/foo.symbols.bak", TestFile::new(PLACEHOLDER))]),
            tree(&[("src/foo.symbols", TestFile::new(PLACEHOLDER))]),
            tree(&[("debian/.pc/foo.symbols", TestFile::new(PLACEHOLDER))]),
            // The final line without a newline loses its last byte, so `#PACKAGE#` no longer matches.
            tree(&[(
                "debian/foo.symbols",
                TestFile::new("* Build-Depends-Package: #PACKAGE#"),
            )]),
            // Symlinks are not inspected.
            tree(&[("debian/foo.symbols", TestFile::symlink("x"))]),
        ];
        let fail = vec![
            tree(&[("debian/foo.symbols", TestFile::new(PLACEHOLDER))]),
            tree(&[(
                "debian/foo.symbols",
                TestFile::new(
                    "libfoo.so.1 libfoo1 #MINVER#\n* Build-Depends-Package: #PACKAGE#-dev\n",
                ),
            )]),
            // The final line without a newline keeps its last byte, so this matches.
            tree(&[(
                "debian/foo.symbols",
                TestFile::new("* Build-Depends-Package: #PACKAGE#x"),
            )]),
            // A hardlink to a regular file counts as a file.
            tree(&[("debian/foo.symbols", TestFile::new(PLACEHOLDER).hardlink())]),
            // Non-UTF-8 bytes elsewhere in the file do not hide the line.
            tree(&[(
                "debian/foo.symbols",
                TestFile::from_bytes(b"\xff\xfe\n* Build-Depends-Package: #PACKAGE#\n".to_vec()),
            )]),
        ];
        SourceTreeTester::new(PackagePlaceholderInSymbolsFile, pass, fail).test_and_snapshot();
    }
}
