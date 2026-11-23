//! Interpreter findings of lintian's `scripts` Check, shared by the Rules that
//! each report one of its Tags.
//!
//! Installed files and Control files (maintainer scripts) are walked the way
//! the Check walks them. A [`Hit`] names its Tag, so each Rule keeps only its
//! own. Only the branches that produce a Tag in this module are here; the
//! other branches of the Check are other Rules.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::{Hashbang, InterpreterVersion, basename, lookup, unversioned};
use crate::lint::package_relation::PackageRelation;

use super::scan;

/// One finding: the Tag, the Location, and the hint extra.
pub(super) struct Hit {
    pub tag: &'static str,
    pub path: PathBuf,
    pub line: Option<u32>,
    pub extra: String,
}

/// Relations that Control-file interpreter checks read.
pub(super) struct ControlFacts {
    pre_depends: PackageRelation,
    strong: PackageRelation,
    multi_arch_same: bool,
}

static PYCOMPILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*py3?compile(?:\s|$)").expect("valid regex"));

/// The Control-file facts for the binary package being checked.
pub(super) fn control_facts(ctx: &BinaryPackageContext<'_>) -> ControlFacts {
    let fields = ctx.installation_control().fields();
    ControlFacts {
        pre_depends: fields.relation("Pre-Depends").unwrap_or_default(),
        strong: fields.relations_and(&["Pre-Depends", "Depends"]),
        multi_arch_same: fields.get("Multi-Arch") == Some("same"),
    }
}

/// lintian's `all_prerequisites`: every dependency field, Provides, and the
/// package's own name, AND-ed.
pub(super) fn installed_prerequisites(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    let fields = ctx.installation_control().fields();
    fields
        .relations_and(&["Pre-Depends", "Depends", "Recommends", "Suggests"])
        .and(&fields.relation("Provides").unwrap_or_default())
        .and(&PackageRelation::parse(ctx.package_name()))
}

/// Hits for installed files: the dependency check for a known script
/// interpreter, the php branch, and the nodejs example branch.
pub(super) fn installed(files: &FileTree, all: &PackageRelation) -> Vec<Hit> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .flat_map(|entry| match files.read(entry) {
            Ok(bytes) => installed_entry(entry, &bytes, all),
            Err(_) => Vec::new(),
        })
        .collect()
}

fn installed_entry(entry: &Entry, bytes: &[u8], all: &PackageRelation) -> Vec<Hit> {
    let Some(name) = entry.path().to_str() else {
        return Vec::new();
    };
    let Some(hashbang) = Hashbang::read(bytes) else {
        return Vec::new();
    };
    let examples = is_examples(name);
    if (name.starts_with("usr/share/doc/") || name.starts_with("usr/src/")) && !examples {
        return Vec::new();
    }
    let interpreter = hashbang.interpreter();
    let base = basename(&interpreter);
    if base == "python" && name.starts_with("usr/lib/python3/dist-packages/") {
        return Vec::new();
    }
    if name.ends_with(".in") && is_placeholder(&interpreter) {
        return Vec::new();
    }
    let absolute = interpreter.starts_with('/') || hashbang.calls_env();
    let executable = entry.is_executable_file();
    if !(name.contains("bin/") || name.contains("etc/init.d/"))
        && !executable
        && !absolute
        && !examples
    {
        return Vec::new();
    }
    if interpreter.is_empty() {
        return Vec::new();
    }

    let path = entry.path().to_path_buf();
    let hit = |tag: &'static str, extra: String| Hit {
        tag,
        path: path.clone(),
        line: None,
        extra,
    };
    let mut hits = Vec::new();
    match lookup(base) {
        Some(known) => {
            // The dependency check runs for unversioned interpreters only, on
            // executable files outside /usr/share/doc and /usr/src.
            if known.version == InterpreterVersion::Unversioned
                && executable
                && base == "python3"
                && !name.starts_with("usr/share/doc/")
                && !name.starts_with("usr/src/")
                && !known.prerequisites.is_empty()
                && !all.satisfies(&known.prerequisites)
            {
                hits.push(hit(
                    "python3-script-but-no-python3-dep",
                    format!("{interpreter} (does not satisfy {})", known.prerequisites),
                ));
            }
        }
        None => {
            if interpreter == "nodejs" && examples {
                hits.push(hit(
                    "example-script-uses-deprecated-nodejs-location",
                    interpreter.clone(),
                ));
            } else if base.starts_with("php") && !examples {
                hits.push(hit(
                    "php-script-with-unusual-interpreter",
                    interpreter.clone(),
                ));
            }
        }
    }
    hits
}

/// Hits for installed scripts under `usr/share/doc/<package>/examples/`: the
/// example variants of the `scripts` Check's interpreter Tags. The Check
/// emits its example Tags only for these paths, in the same order as the
/// non-example ones. Each Hit names its Tag, and each Rule keeps its own.
pub(super) fn examples(files: &FileTree) -> Vec<Hit> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .flat_map(|entry| match files.read(entry) {
            Ok(bytes) => example_entry(files, entry, &bytes),
            Err(_) => Vec::new(),
        })
        .collect()
}

fn example_entry(files: &FileTree, entry: &Entry, bytes: &[u8]) -> Vec<Hit> {
    let Some(name) = entry.path().to_str() else {
        return Vec::new();
    };
    if !is_examples(name) {
        return Vec::new();
    }
    let Some(hashbang) = Hashbang::read(bytes) else {
        return Vec::new();
    };
    let interpreter = hashbang.interpreter();
    if name.ends_with(".in") && is_placeholder(&interpreter) {
        return Vec::new();
    }
    let absolute = interpreter.starts_with('/') || hashbang.calls_env();
    let path = entry.path().to_path_buf();
    let hit = |tag: &'static str, extra: String| Hit {
        tag,
        path: path.clone(),
        line: None,
        extra,
    };
    let mut hits = Vec::new();
    if hashbang.calls_env() && interpreter == "perl" {
        hits.push(hit(
            "example-incorrect-path-for-interpreter",
            "/usr/bin/env perl != /usr/bin/perl".to_string(),
        ));
    }
    // Example Perl modules and editor hints that are not executable and
    // not absolute are not scripts.
    if name.ends_with(".pm") && !entry.is_executable_file() && !absolute {
        return hits;
    }
    if interpreter.is_empty() {
        hits.push(hit("example-script-without-interpreter", String::new()));
        return hits;
    }
    if !absolute {
        hits.push(hit("example-interpreter-not-absolute", interpreter.clone()));
    }
    let base = basename(&interpreter);
    match lookup(base) {
        Some(known) => {
            if !hashbang.calls_env()
                && let Some(extra) = example_mismatch(&interpreter, known.folder, base)
            {
                hits.push(hit("example-wrong-path-for-interpreter", extra));
            }
        }
        None => {
            // `/usr/local/` interpreters are another Tag's example; only
            // `/bin/env` is reported here.
            if interpreter == "/bin/env" {
                hits.push(hit("example-script-uses-bin-env", interpreter.clone()));
            } else if interpreter != "nodejs"
                && !interpreter.starts_with("/usr/local/")
                && !base.starts_with("php")
                && !shipped_executable(files, &interpreter, hashbang.calls_env())
            {
                hits.push(hit("example-unusual-interpreter", interpreter.clone()));
            }
        }
    }
    hits
}

/// Whether the package ships an executable file for `interpreter`, as the
/// `scripts` Check's private-interpreter lookup: the path without its leading
/// slash, or `bin/<name>` and `usr/bin/<name>` after `/usr/bin/env`.
fn shipped_executable(files: &FileTree, interpreter: &str, calls_env: bool) -> bool {
    let candidates: Vec<String> = if let Some(path) = interpreter.strip_prefix('/') {
        vec![path.to_string()]
    } else if calls_env {
        vec![
            format!("bin/{interpreter}"),
            format!("usr/bin/{interpreter}"),
        ]
    } else {
        Vec::new()
    };
    candidates.iter().any(|path| {
        files
            .get(path)
            .is_some_and(|entry| entry.is_file() && entry.is_executable_file())
    })
}

/// The extra when an example's interpreter is not at the table folder, as
/// the `scripts` Check's `$expected` test. A table folder under `/usr/bin/`,
/// `/usr/sbin/`, or `/usr/lib/` also accepts the path without `/usr` (not
/// for perl). `/usr/bin/env perl` as the expected path is reported by the
/// incorrect-path branch instead, and no table folder is `/usr/bin/env`.
fn example_mismatch(interpreter: &str, folder: &str, basename: &str) -> Option<String> {
    let expected = format!("{folder}/{basename}");
    let merged = ["/usr/bin/", "/usr/sbin/", "/usr/lib/"]
        .iter()
        .any(|prefix| expected.starts_with(prefix));
    if merged && basename != "perl" {
        let non_usr = &expected["/usr".len()..];
        let accepted = interpreter.starts_with(non_usr) || interpreter.starts_with(&expected);
        return (!accepted).then(|| format!("{interpreter} != {non_usr} OR {expected}"));
    }
    (!interpreter.contains(&expected)).then(|| format!("{interpreter} != {expected}"))
}

/// Hits for Control files (maintainer scripts): the interpreter checks and,
/// for shell scripts that the Check reads, the pycompile line check.
pub(super) fn control(files: &FileTree, facts: &ControlFacts) -> Vec<Hit> {
    let mut hits = Vec::new();
    // Maintainer scripts whose body the Check reads.
    let mut scanned = HashSet::new();
    for entry in files.query().kinds([EntryKind::RegularFile]).into_iter() {
        let Some(name) = entry.path().to_str() else {
            continue;
        };
        if !matches!(name, "preinst" | "postinst" | "prerm" | "postrm" | "config") {
            continue;
        }
        if matches!(entry.file_type(), Some(FileType::Elf { .. })) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let Some(token) = scan::shebang_token(&bytes) else {
            continue;
        };
        let path = entry.path().to_path_buf();
        let base = basename(&token);
        let hit = |tag: &'static str, extra: String| Hit {
            tag,
            path: path.clone(),
            line: None,
            extra,
        };
        if token.starts_with("/usr/local/") {
            hits.push(hit("control-interpreter-in-usr-local", token.clone()));
        } else if matches!(base, "sh" | "bash" | "perl") || name == "config" || name == "postrm" {
            // Other Tags; the body is still read.
        } else if let Some(known) = unversioned(base) {
            hits.push(hit("unusual-control-interpreter", token.clone()));
            if !known.prerequisites.is_empty() {
                let depends = PackageRelation::parse(&known.prerequisites).to_lintian_string();
                let extra = format!("{token} (does not satisfy {depends})");
                if name == "preinst" {
                    if !facts.pre_depends.satisfies(&known.prerequisites) {
                        hits.push(hit("control-interpreter-without-predepends", extra));
                    }
                } else if !facts.strong.satisfies(&known.prerequisites) {
                    hits.push(hit("control-interpreter-without-depends", extra));
                }
            }
        } else {
            // An unknown interpreter: the Check stops before the body.
            continue;
        }
        scanned.insert(path);
    }

    if facts.multi_arch_same {
        hits.extend(
            scan::script_lines(files)
                .into_iter()
                .filter(|line| line.shell && scanned.contains(&line.path))
                .filter(|line| PYCOMPILE.is_match(&line.text))
                .map(|line| Hit {
                    tag: "multi-arch-same-package-calls-pycompile",
                    path: line.path,
                    line: Some(line.line),
                    extra: String::new(),
                }),
        );
    }
    hits
}

/// `usr/share/doc/<package>/examples/…`.
fn is_examples(name: &str) -> bool {
    name.strip_prefix("usr/share/doc/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(package, after)| !package.is_empty() && after.starts_with("examples/"))
}

/// A build placeholder such as `@PERL@` or `<<PERL>>`.
fn is_placeholder(interpreter: &str) -> bool {
    let start = interpreter
        .strip_prefix('@')
        .or_else(|| interpreter.strip_prefix("<<"));
    let inner = start.and_then(|rest| rest.strip_suffix('@').or_else(|| rest.strip_suffix(">>")));
    inner.is_some_and(|inner| {
        !inner.is_empty()
            && inner
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_are_under_doc_package_examples() {
        assert!(is_examples("usr/share/doc/foo/examples/bar"));
        assert!(!is_examples("usr/share/doc/foo/bar"));
        assert!(!is_examples("usr/share/doc//examples/bar"));
    }

    #[test]
    fn placeholders_are_build_tokens() {
        assert!(is_placeholder("@PERL@"));
        assert!(is_placeholder("<<PERL_BIN>>"));
        assert!(!is_placeholder("@perl@"));
        assert!(!is_placeholder("/usr/bin/perl"));
    }
}
