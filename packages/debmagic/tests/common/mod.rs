//! Shared helpers for `debmagic lint` integration tests.

#![allow(dead_code)]

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

pub fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets")
}

pub fn run_lint(source_dir: &Path, extra_args: &[&str]) -> (ExitStatus, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_debmagic"))
        .arg("lint")
        .arg("--source-dir")
        .arg(source_dir)
        .args(extra_args)
        .output()
        .expect("failed to spawn debmagic");

    (
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

pub fn run_lint_subject(subject: &Path, extra_args: &[&str]) -> (ExitStatus, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_debmagic"))
        .arg("lint")
        .arg(subject)
        .args(extra_args)
        .output()
        .expect("failed to spawn debmagic");

    (
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

pub fn temp_artifact(extension: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "debmagic-lint-integration-{}-{}.{extension}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, b"").expect("writing temp artifact");
    path
}

pub fn parity_suite_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/lintian-parity")
}

fn discover_parity_recipes(root: &Path) -> Vec<PathBuf> {
    let mut recipes = Vec::new();
    fn walk(dir: &Path, recipes: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join("ORIGIN").is_file() {
                recipes.push(path);
                continue;
            }
            walk(&path, recipes);
        }
    }
    walk(root, &mut recipes);
    recipes.sort();
    recipes
}

/// Tags of catalogued LN Rules, read from `src/lint/rules/lintian/`.
pub fn catalogued_ln_tags() -> HashSet<String> {
    let mut tags = HashSet::new();
    let rules_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lint/rules/lintian");
    fn walk(dir: &Path, tags: &mut HashSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, tags);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                let Some(rest) = line.strip_prefix("tag = \"") else {
                    continue;
                };
                let Some(tag) = rest.strip_suffix("\",") else {
                    continue;
                };
                tags.insert(tag.to_string());
            }
        }
    }
    walk(&rules_dir, &mut tags);
    tags
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParityHint {
    pub tag: String,
    pub extra: String,
}

fn strip_location_pointer(extra: &str) -> String {
    extra
        .rsplit_once(" [")
        .and_then(|(before, pointer)| pointer.ends_with(']').then_some(before))
        .unwrap_or(extra)
        .to_string()
}

pub fn parse_universal_hint_line(line: &str) -> Option<(String, ParityHint)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (package_and_type, rest) = line.split_once(": ")?;
    let (_package, type_part) = package_and_type.rsplit_once(" (")?;
    let processable_type = type_part.strip_suffix(')')?;
    let mut fields = rest.splitn(2, char::is_whitespace);
    let tag = fields.next().filter(|tag| !tag.is_empty())?;
    let extra = strip_location_pointer(fields.next().unwrap_or("").trim());
    Some((
        processable_type.to_string(),
        ParityHint {
            tag: tag.to_string(),
            extra,
        },
    ))
}

pub fn parse_diagnostic_line(line: &str) -> Option<(String, ParityHint)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("All checks") || line.starts_with("Found ") {
        return None;
    }
    let rest = ["E:", "W:", "I:", "P:"]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))?;
    let rest = rest.trim();
    let (package_and_type, after) = rest.split_once(": ")?;
    let (_package, type_part) = package_and_type.rsplit_once(" (")?;
    let processable_type = type_part.strip_suffix(')')?;
    let (tag, after_tag) = after.split_once(' ')?;
    let after_code = after_tag.strip_prefix('(')?;
    let (_code, extra) = after_code.split_once(')')?;
    let extra = strip_location_pointer(extra.trim());
    Some((
        processable_type.to_string(),
        ParityHint {
            tag: tag.to_string(),
            extra,
        },
    ))
}

fn applies_to_source_tree(processable_type: &str, extra: &str) -> bool {
    processable_type == "source"
        && extra
            .split_whitespace()
            .next()
            .is_none_or(|first| !first.ends_with(".dsc"))
}

fn applies_to_binary_package(processable_type: &str) -> bool {
    processable_type == "binary" || processable_type == "udeb"
}

fn load_hints(path: &Path) -> Vec<(String, ParityHint)> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(parse_universal_hint_line)
        .collect()
}

fn recipe_is_source_tree(recipe: &Path) -> bool {
    recipe.join("debian").is_dir()
}

fn recipe_is_deb_skeleton(recipe: &Path) -> bool {
    let Ok(origin) = fs::read_to_string(recipe.join("ORIGIN")) else {
        return false;
    };
    origin.lines().any(|line| line.trim() == "Skeleton: deb")
}

fn recipe_rel(root: &Path, recipe: &Path) -> String {
    recipe
        .strip_prefix(root)
        .unwrap_or(recipe)
        .to_str()
        .expect("parity recipe path is UTF-8")
        .replace('\\', "/")
}

fn catalogued_source_tree_hints(recipe: &Path, catalogued: &HashSet<String>) -> Vec<ParityHint> {
    let mut expected: Vec<ParityHint> = load_hints(&recipe.join("eval/hints"))
        .into_iter()
        .filter(|(processable_type, hint)| {
            catalogued.contains(&hint.tag) && applies_to_source_tree(processable_type, &hint.extra)
        })
        .map(|(_, hint)| hint)
        .collect();
    expected.sort();
    expected
}

/// Vendored Source-tree recipes whose hints mention a catalogued LN Tag.
pub fn catalogued_source_tree_recipe_rels() -> Vec<String> {
    let root = parity_suite_root();
    let catalogued = catalogued_ln_tags();
    let mut rels = Vec::new();
    for recipe in discover_parity_recipes(&root) {
        if !recipe_is_source_tree(&recipe) {
            continue;
        }
        if catalogued_source_tree_hints(&recipe, &catalogued).is_empty() {
            continue;
        }
        rels.push(recipe_rel(&root, &recipe));
    }
    rels
}

pub fn assert_source_tree_parity(recipe_rel: &str) {
    let recipe = parity_suite_root().join(recipe_rel);
    let catalogued = catalogued_ln_tags();
    let expected = catalogued_source_tree_hints(&recipe, &catalogued);
    assert!(
        !expected.is_empty(),
        "{recipe_rel}: listed Parity recipe has no catalogued Source-tree hints"
    );

    let mut selected: Vec<String> = expected
        .iter()
        .map(|hint| hint.tag.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    selected.sort();
    let mut extra_args = Vec::new();
    for tag in &selected {
        extra_args.push("--select");
        extra_args.push(tag.as_str());
    }

    let (_status, stdout, stderr) = run_lint(&recipe, &extra_args);
    assert!(
        stderr.is_empty() || !stdout.is_empty(),
        "{recipe_rel}: lint failed with empty stdout\n{stderr}"
    );

    let mut actual: Vec<ParityHint> = stdout
        .lines()
        .filter_map(parse_diagnostic_line)
        .filter(|(processable_type, hint)| {
            catalogued.contains(&hint.tag) && applies_to_source_tree(processable_type, &hint.extra)
        })
        .map(|(_, hint)| hint)
        .collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (source tree)"
    );
}

fn catalogued_binary_package_hints(recipe: &Path, catalogued: &HashSet<String>) -> Vec<ParityHint> {
    let mut expected: Vec<ParityHint> = load_hints(&recipe.join("eval/hints"))
        .into_iter()
        .filter(|(processable_type, hint)| {
            catalogued.contains(&hint.tag) && applies_to_binary_package(processable_type)
        })
        .map(|(_, hint)| hint)
        .collect();
    expected.sort();
    expected
}

/// Vendored `deb`-skeleton recipes whose hints mention a catalogued LN Tag.
pub fn catalogued_binary_package_recipe_rels() -> Vec<String> {
    let root = parity_suite_root();
    let catalogued = catalogued_ln_tags();
    let mut rels = Vec::new();
    for recipe in discover_parity_recipes(&root) {
        if !recipe_is_deb_skeleton(&recipe) {
            continue;
        }
        if catalogued_binary_package_hints(&recipe, &catalogued).is_empty() {
            continue;
        }
        rels.push(recipe_rel(&root, &recipe));
    }
    rels
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap_or_else(|error| panic!("creating {}: {error}", dst.display()));
    for entry in
        fs::read_dir(src).unwrap_or_else(|error| panic!("reading {}: {error}", src.display()))
    {
        let entry = entry.expect("directory entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            copy_tree(&from, &to);
        } else if file_type.is_file() {
            fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!("copying {} to {}: {error}", from.display(), to.display())
            });
        }
    }
}

fn find_binary_package(dir: &Path) -> PathBuf {
    let mut found = Vec::new();
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if let Some("deb" | "udeb" | "ddeb") = path.extension().and_then(|ext| ext.to_str()) {
            found.push(path);
        }
    }
    assert_eq!(
        found.len(),
        1,
        "expected one Build-Product in {}, found {found:?}",
        dir.display()
    );
    found.remove(0)
}

pub fn assert_binary_package_parity(recipe_rel: &str) {
    let recipe = parity_suite_root().join(recipe_rel);
    let catalogued = catalogued_ln_tags();
    let expected = catalogued_binary_package_hints(&recipe, &catalogued);
    assert!(
        !expected.is_empty(),
        "{recipe_rel}: listed Parity recipe has no catalogued Binary-package hints"
    );

    let work = tempfile::tempdir().expect("temp recipe workdir");
    let source = work.path().join("src");
    let build = work.path().join("build");
    copy_tree(&recipe, &source);
    fs::create_dir_all(&build).expect("creating build dir");
    let output = Command::new("make")
        .arg("-C")
        .arg(&build)
        .arg("-f")
        .arg(source.join("Makefile"))
        .output()
        .expect("spawning make");
    assert!(
        output.status.success(),
        "{recipe_rel}: make failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let subject = find_binary_package(&build);

    let mut selected: Vec<String> = expected
        .iter()
        .map(|hint| hint.tag.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    selected.sort();
    let mut extra_args = Vec::new();
    for tag in &selected {
        extra_args.push("--select");
        extra_args.push(tag.as_str());
    }

    let (_status, stdout, stderr) = run_lint_subject(&subject, &extra_args);
    assert!(
        stderr.is_empty() || !stdout.is_empty(),
        "{recipe_rel}: lint failed with empty stdout\n{stderr}"
    );

    let mut actual: Vec<ParityHint> = stdout
        .lines()
        .filter_map(parse_diagnostic_line)
        .filter(|(processable_type, hint)| {
            catalogued.contains(&hint.tag) && applies_to_binary_package(processable_type)
        })
        .map(|(_, hint)| hint)
        .collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (binary package)"
    );
}
