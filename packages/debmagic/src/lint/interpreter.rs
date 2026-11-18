//! Script interpreters: the `#!` line of an Entry and lintian's interpreter
//! table.
//!
//! The table is lintian 2.139.0 `data/scripts/interpreters` (unversioned
//! names) and `data/scripts/versioned-interpreters` (names that carry a
//! version, such as `pike7.6` or `guile-3.0`). It maps an interpreter's
//! basename to the folder it is installed in and to the relation a package
//! needs to obtain it. The lookup follows lintian's `scripts` Check: the
//! unversioned table first, then a versioned family by its exact name, then
//! the family left after stripping a trailing version. There is no Script
//! type; Rules read the bytes of an Entry and call [`Hashbang::read`].

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

/// The text after `#!` on the first line of a script, lintian's `hashbang`.
///
/// Both ends are trimmed of whitespace. A file that is only `#!` is a script
/// with an empty hashbang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hashbang {
    line: Vec<u8>,
}

impl Hashbang {
    /// lintian's `is_script` and `hashbang`: `None` when the content does not
    /// start with `#!`, or starts with `#!#!#!` (lincity data files).
    pub(crate) fn read(content: &[u8]) -> Option<Self> {
        if !content.starts_with(b"#!") || content.starts_with(b"#!#!#!") {
            return None;
        }
        let rest = &content[2..];
        let line = match rest.iter().position(|&byte| byte == b'\n') {
            Some(end) => &rest[..end],
            None => rest,
        };
        Some(Self {
            line: trim(line).to_vec(),
        })
    }

    /// lintian's `calls_env`: the hashbang starts with `/usr/bin/env` and
    /// whitespace.
    pub(crate) fn calls_env(&self) -> bool {
        self.line
            .strip_prefix(b"/usr/bin/env".as_slice())
            .is_some_and(|after| after.first().is_some_and(|&byte| is_space(byte)))
    }

    /// lintian's `interpreter`: the first word after a leading
    /// `/usr/bin/env` and its whitespace are removed. Options are dropped.
    /// `#!/usr/bin/env python3 -u` is `python3`; `#!/bin/env perl` is
    /// `/bin/env`. Empty when the hashbang is.
    pub(crate) fn interpreter(&self) -> String {
        let mut rest = self.line.as_slice();
        if let Some(after) = rest.strip_prefix(b"/usr/bin/env".as_slice())
            && after.first().is_some_and(|&byte| is_space(byte))
        {
            rest = trim(after);
        }
        let end = rest
            .iter()
            .position(|&byte| is_space(byte))
            .unwrap_or(rest.len());
        String::from_utf8_lossy(&rest[..end]).into_owned()
    }
}

/// Perl's `File::Basename::basename`: the last path segment, trailing
/// slashes ignored.
pub(crate) fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "" } else { "/" };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// A table entry that applies to an interpreter basename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KnownInterpreter {
    /// Folder the interpreter is installed in (`/usr/bin`). The expected
    /// path is this folder, a slash, and the basename.
    #[allow(dead_code)]
    pub folder: &'static str,
    /// Relation that provides the interpreter, as lintian builds the text.
    /// Empty when no prerequisite is needed (an essential package).
    pub prerequisites: String,
    pub version: InterpreterVersion,
}

/// Which part of the table matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InterpreterVersion {
    /// `scripts/interpreters`.
    Unversioned,
    /// A `scripts/versioned-interpreters` family by its own name (`pike`):
    /// any listed version, or the family's own relation, will do.
    AnyVersion,
    /// A versioned name of a family (`pike7.6`), with the captured version.
    Version(String),
}

/// lintian's lookup of an interpreter basename. `None` for an unknown
/// interpreter.
pub(crate) fn lookup(basename: &str) -> Option<KnownInterpreter> {
    if let Some(known) = unversioned(basename) {
        return Some(known);
    }
    if let Some(family) = VERSIONED.get(basename) {
        return Some(KnownInterpreter {
            folder: family.folder,
            prerequisites: family.any_version(),
            version: InterpreterVersion::AnyVersion,
        });
    }
    let stripped = VERSION_SUFFIX.captures(basename)?;
    let family = VERSIONED.get(stripped.get(1)?.as_str())?;
    let captures = family.pattern.captures(basename)?;
    let version = captures.get(1).map_or("", |found| found.as_str());
    Some(KnownInterpreter {
        folder: family.folder,
        prerequisites: family.template.replace("$1", version),
        version: InterpreterVersion::Version(version.to_string()),
    })
}

/// Only `scripts/interpreters`. lintian falls back to this for `nodejs`
/// (`node`) and for unusual `php…` names (`php`).
pub(crate) fn unversioned(basename: &str) -> Option<KnownInterpreter> {
    UNVERSIONED.get(basename).map(|entry| KnownInterpreter {
        folder: entry.folder,
        prerequisites: entry.prerequisites.to_string(),
        version: InterpreterVersion::Unversioned,
    })
}

struct Unversioned {
    folder: &'static str,
    prerequisites: &'static str,
}

struct Family {
    folder: &'static str,
    pattern: Regex,
    template: &'static str,
    versions: Vec<&'static str>,
    /// `None` for `@SKIP_UNVERSIONED@`.
    prerequisites: Option<&'static str>,
}

impl Family {
    /// The family's relation, then the template for each listed version,
    /// joined with ` | `.
    fn any_version(&self) -> String {
        self.prerequisites
            .filter(|relation| !relation.is_empty())
            .map(str::to_string)
            .into_iter()
            .chain(
                self.versions
                    .iter()
                    .map(|version| self.template.replace("$1", version)),
            )
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// A versioned basename: the family name, an optional dash, and a version.
static VERSION_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*[^\d.-])-?[\d.]+$").expect("valid regex"));

static FIELD_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));

/// lintian 2.139.0 `data/scripts/interpreters`.
static UNVERSIONED: LazyLock<HashMap<&'static str, Unversioned>> = LazyLock::new(|| {
    entries(include_str!("data/scripts/interpreters"))
        .map(|(name, value)| {
            let mut fields = FIELD_SEPARATOR.splitn(value, 2);
            let folder = fields.next().unwrap_or_default();
            let prerequisites = fields.next().unwrap_or_default();
            (
                name,
                Unversioned {
                    folder,
                    prerequisites,
                },
            )
        })
        .collect()
});

/// lintian 2.139.0 `data/scripts/versioned-interpreters`.
static VERSIONED: LazyLock<HashMap<&'static str, Family>> = LazyLock::new(|| {
    entries(include_str!("data/scripts/versioned-interpreters"))
        .map(|(name, value)| {
            let fields: Vec<&str> = FIELD_SEPARATOR.splitn(value, 5).collect();
            let field = |index: usize| fields.get(index).copied().unwrap_or_default();
            let pattern = Regex::new(&format!("^(?:{})$", field(1)))
                .unwrap_or_else(|error| panic!("versioned-interpreters {name}: {error}"));
            let prerequisites = match field(4) {
                "@SKIP_UNVERSIONED@" => None,
                other => Some(other),
            };
            (
                name,
                Family {
                    folder: field(0),
                    pattern,
                    template: field(2),
                    versions: field(3).split_whitespace().collect(),
                    prerequisites,
                },
            )
        })
        .collect()
});

/// `name => value` lines of a lintian data file.
fn entries(text: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    crate::lint::data::lines(text).filter_map(|line| {
        let (name, value) = line.split_once("=>")?;
        Some((name.trim(), value.trim()))
    })
}

/// Perl's `\s` on a byte string with `unicode_strings` (lintian's `use v5.20`).
fn is_space(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | 0x85 | 0xa0
    )
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&byte| !is_space(byte))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|&byte| !is_space(byte))
        .map_or(start, |last| last + 1);
    &bytes[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interpreter(content: &str) -> Option<String> {
        Hashbang::read(content.as_bytes()).map(|hashbang| hashbang.interpreter())
    }

    #[test]
    fn hashbang_follows_lintian() {
        assert_eq!(interpreter("no shebang\n"), None);
        assert_eq!(interpreter("#!#!#!\n"), None);
        assert_eq!(interpreter("#!").as_deref(), Some(""));
        assert_eq!(interpreter("#!\n").as_deref(), Some(""));
        assert_eq!(interpreter("#! /bin/csh\n\n").as_deref(), Some("/bin/csh"));
        assert_eq!(
            interpreter("#!/usr/bin/python3 -u\nprint(1)\n").as_deref(),
            Some("/usr/bin/python3")
        );
        assert_eq!(
            interpreter("#!/usr/bin/env node\n").as_deref(),
            Some("node")
        );
        assert_eq!(
            interpreter("#!/usr/bin/env  python3 -u\n").as_deref(),
            Some("python3")
        );
        // `env` without a following word stays.
        assert_eq!(
            interpreter("#!/usr/bin/env\n").as_deref(),
            Some("/usr/bin/env")
        );
        assert_eq!(
            interpreter("#!/bin/env perl\n").as_deref(),
            Some("/bin/env")
        );
        assert_eq!(interpreter("#!/bin/sh\r\n").as_deref(), Some("/bin/sh"));
        assert_eq!(
            Hashbang::read(b"#!/bin/sh\xa0-e\n").map(|hashbang| hashbang.interpreter()),
            Some("/bin/sh".to_string())
        );
    }

    #[test]
    fn basename_follows_file_basename() {
        assert_eq!(basename("/usr/bin/pike7.6"), "pike7.6");
        assert_eq!(basename("nodejs"), "nodejs");
        assert_eq!(basename("/usr/bin/"), "bin");
        assert_eq!(basename(""), "");
        assert_eq!(basename("/"), "/");
    }

    #[test]
    fn unversioned_entries() {
        let lefty = lookup("lefty").expect("lefty");
        assert_eq!(lefty.folder, "/usr/bin");
        assert_eq!(lefty.prerequisites, "graphviz:any");
        assert_eq!(lefty.version, InterpreterVersion::Unversioned);
        let sh = lookup("sh").expect("sh");
        assert_eq!(sh.folder, "/bin");
        assert_eq!(sh.prerequisites, "");
        assert_eq!(
            lookup("run").expect("run").prerequisites,
            "runit:any (>= 2.1.2-23)"
        );
        assert_eq!(
            lookup("Rscript").expect("Rscript").prerequisites,
            "r-base-core:any"
        );
        assert_eq!(lookup("rscript"), None);
        // An unversioned entry wins over a versioned family of the same name.
        assert_eq!(
            lookup("python3").expect("python3").prerequisites,
            "python3:any | python3-minimal:any"
        );
        assert_eq!(unversioned("pike"), None);
    }

    #[test]
    fn versioned_family_by_name() {
        let pike = lookup("pike").expect("pike");
        assert_eq!(pike.version, InterpreterVersion::AnyVersion);
        assert_eq!(
            pike.prerequisites,
            "pike7.6:any | pike7.6-core:any | pike7.8:any | pike7.8-core:any"
        );
        assert_eq!(
            lookup("jruby").expect("jruby").prerequisites,
            "jruby:any | jruby1.0:any | jruby1.1:any | jruby1.2:any"
        );
    }

    #[test]
    fn versioned_name() {
        let pike = lookup("pike7.6").expect("pike7.6");
        assert_eq!(pike.folder, "/usr/bin");
        assert_eq!(pike.version, InterpreterVersion::Version("7.6".to_string()));
        assert_eq!(pike.prerequisites, "pike7.6:any | pike7.6-core:any");
        // The version list is not consulted for a versioned name.
        assert_eq!(
            lookup("guile-9.9").expect("guile-9.9").prerequisites,
            "guile-9.9:any"
        );
        assert_eq!(
            lookup("ruby1.8").expect("ruby1.8").prerequisites,
            "ruby1.8:any"
        );
        // The family's pattern must match the whole name.
        assert_eq!(lookup("guile3.0"), None);
        // `python3.7` strips to `python`, which is no family.
        assert_eq!(lookup("python3.7"), None);
        assert_eq!(lookup("lintian"), None);
    }
}
