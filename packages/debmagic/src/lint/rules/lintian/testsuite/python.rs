//! The `py3versions` checks of a `debian/tests/control` paragraph.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::Location;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::package_relation::PackageRelation;

use super::sections::{Section, resolve};

const PYTHON3_ALL_DEPEND: &str = "python3-all:any | python3-all-dev:any | python3-all-dbg:any";

static PY3VERSIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(py3versions)((?:\s+--?\w+)*)").expect("valid regex"));
static SUPPORTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s(?:-\w*s|--supported)").expect("valid regex"));
static REQUESTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s(?:-\w*r|--requested)").expect("valid regex"));
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));
static COMMAS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",+").expect("valid regex"));

/// A `py3versions` call in a test file.
pub(super) struct Call {
    pub(super) location: Location,
    /// `py3versions` and its options, as lintian reports it.
    pub(super) command: String,
    options: String,
}

impl Call {
    pub(super) fn asks_supported(&self) -> bool {
        SUPPORTED.is_match(&self.options)
    }

    pub(super) fn asks_requested(&self) -> bool {
        REQUESTED.is_match(&self.options)
    }
}

pub(super) struct Scan {
    pub(super) calls: Vec<Call>,
    /// Every test file that could be read asks for the supported versions
    /// (also true with no readable test file).
    pub(super) all_tests_use_supported: bool,
}

/// `py3versions` calls in the regular files named by `section`'s `Tests`.
pub(super) fn scan(files: &FileTree, section: &Section) -> Scan {
    let mut calls = Vec::new();
    let mut all_tests_use_supported = true;
    for path in section.test_file_paths() {
        let Some(entry) = resolve(files, &path) else {
            continue;
        };
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut queries_supported = false;
        for (index, line) in text.lines().enumerate() {
            let Some(captures) = PY3VERSIONS.captures(line) else {
                continue;
            };
            let options = captures.get(2).map_or("", |m| m.as_str());
            let call = Call {
                location: Location {
                    path: PathBuf::from(entry.path()),
                    line: Some(u32::try_from(index + 1).unwrap_or(u32::MAX)),
                    column: None,
                },
                command: format!("py3versions{options}"),
                options: options.to_string(),
            };
            queries_supported |= call.asks_supported();
            calls.push(call);
        }
        all_tests_use_supported &= queries_supported;
    }
    Scan {
        calls,
        all_tests_use_supported,
    }
}

/// The `Depends` of `section` with `@builddeps@` and `@recommends@` expanded,
/// as lintian builds it for the `python3-all` check.
fn depends(section: &Section, control: &DebianControl) -> PackageRelation {
    let mut depends = section.unfolded_value("Depends");
    if depends.contains("@builddeps@") {
        let source = control.source();
        let all = [
            source.unfolded_value("Build-Depends").unwrap_or_default(),
            source
                .unfolded_value("Build-Depends-Indep")
                .unwrap_or_default(),
            source
                .unfolded_value("Build-Depends-Arch")
                .unwrap_or_default(),
            "build-essential".to_string(),
        ]
        .join(",");
        depends = depends.replacen("@builddeps@", &all, 1);
    }
    if depends.contains("@recommends@") {
        let mut recommends = String::new();
        for installable in control.installables() {
            let value = installable
                .fields()
                .unfolded_value("Recommends")
                .unwrap_or_default();
            if !value.is_empty() {
                recommends.push(',');
                recommends.push_str(&value);
            }
        }
        depends = depends.replacen("@recommends@", &recommends, 1);
    }
    let depends = SPACE.replace_all(&depends, "");
    let depends = COMMAS.replace_all(&depends, ",");
    PackageRelation::parse(&depends)
}

/// The expanded `Depends` implies one of the `python3-all` packages.
pub(super) fn depends_on_python3_all(section: &Section, control: &DebianControl) -> bool {
    depends(section, control).satisfies(PYTHON3_ALL_DEPEND)
}
