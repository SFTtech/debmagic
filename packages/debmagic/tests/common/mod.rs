//! Shared helpers for `debmagic lint` integration tests.

#![allow(dead_code)]

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
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

pub fn build_native_dsc(work: &Path, name: &str, extra_files: &[(&str, &[u8])]) -> PathBuf {
    let tree = work.join("src");
    fs::create_dir_all(tree.join("debian/source")).expect("debian/source");
    fs::write(
        tree.join("debian/control"),
        format!(
            "Source: {name}\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: {name}\n\
             Architecture: all\n\
             Description: example\n extra\n"
        ),
    )
    .expect("control");
    fs::write(
        tree.join("debian/changelog"),
        format!(
            "{name} (1.0) unstable; urgency=low\n\n  * test\n\n -- a <a@localhost>  Tue, 30 Dec 2008 17:34:02 -0800\n"
        ),
    )
    .expect("changelog");
    let rules = tree.join("debian/rules");
    fs::write(&rules, "#!/usr/bin/make -f\n").expect("rules");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&rules).expect("rules metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&rules, permissions).expect("chmod rules");
    }
    fs::write(tree.join("debian/source/format"), "3.0 (native)\n").expect("format");
    for (relative, contents) in extra_files {
        let path = tree.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("extra file parent");
        }
        fs::write(path, contents).expect("extra file");
    }
    let output = Command::new("dpkg-source")
        .current_dir(work)
        .args(["-b", "src"])
        .output()
        .expect("spawning dpkg-source -b");
    assert!(
        output.status.success(),
        "dpkg-source -b failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dsc = work.join(format!("{name}_1.0.dsc"));
    assert!(dsc.is_file(), "missing {}", dsc.display());
    dsc
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

fn applies_to_source_package(processable_type: &str, tag: &str, tags: &HashSet<String>) -> bool {
    tags.contains(tag) && processable_type == "source"
}

fn applies_to_binary_package(processable_type: &str) -> bool {
    processable_type == "binary" || processable_type == "udeb"
}

/// Tags of LN Rules that implement `BinaryPackageRule`.
pub fn catalogued_binary_package_tags() -> HashSet<String> {
    catalogued_tags_with_impl("impl BinaryPackageRule for")
}

/// Tags of LN Rules that implement `SourceRule`.
pub fn catalogued_source_package_tags() -> HashSet<String> {
    catalogued_tags_with_impl("impl SourceRule for")
}

fn catalogued_tags_with_impl(needle: &str) -> HashSet<String> {
    let mut tags = HashSet::new();
    let rules_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lint/rules/lintian");
    fn walk(dir: &Path, needle: &str, tags: &mut HashSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, tags);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(needle) {
                continue;
            }
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
    walk(&rules_dir, needle, &mut tags);
    tags
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

fn recipe_needs_cross_compiler(recipe: &Path) -> bool {
    let Ok(text) = fs::read_to_string(recipe.join("Makefile")) else {
        return false;
    };
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("CC :=")
            && (line.contains("arm-linux-gnueabihf-gcc") || line.contains("x86_64-linux-gnu-gcc"))
    })
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

fn catalogued_source_package_hints(
    recipe: &Path,
    source_package_tags: &HashSet<String>,
) -> Vec<ParityHint> {
    let mut expected: Vec<ParityHint> = load_hints(&recipe.join("eval/hints"))
        .into_iter()
        .filter(|(processable_type, hint)| {
            applies_to_source_package(processable_type, &hint.tag, source_package_tags)
        })
        .map(|(_, hint)| hint)
        .collect();
    expected.sort();
    expected
}

/// Vendored filled Source-tree recipes with catalogued Source-package hints.
pub fn catalogued_source_package_recipe_rels() -> Vec<String> {
    let root = parity_suite_root();
    let tags = catalogued_source_package_tags();
    let mut rels = Vec::new();
    for recipe in discover_parity_recipes(&root) {
        if !recipe_is_source_tree(&recipe) {
            continue;
        }
        if catalogued_source_package_hints(&recipe, &tags).is_empty() {
            continue;
        }
        rels.push(recipe_rel(&root, &recipe));
    }
    rels
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

/// Vendored recipes whose hints mention a catalogued Tag with Binary-package Applicability.
pub fn catalogued_binary_package_recipe_rels() -> Vec<String> {
    let root = parity_suite_root();
    let catalogued = catalogued_binary_package_tags();
    let mut rels = Vec::new();
    for recipe in discover_parity_recipes(&root) {
        if !recipe_is_deb_skeleton(&recipe) && !recipe_is_source_tree(&recipe) {
            continue;
        }
        if recipe_needs_cross_compiler(&recipe) {
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

fn find_binary_packages(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if !path.is_file() {
            continue;
        }
        if let Some("deb" | "udeb" | "ddeb") = path.extension().and_then(|ext| ext.to_str()) {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn find_binary_package(dir: &Path) -> PathBuf {
    let found = find_binary_packages(dir);
    assert_eq!(
        found.len(),
        1,
        "expected one Build-Product in {}, found {found:?}",
        dir.display()
    );
    found.into_iter().next().expect("one Build-Product")
}

fn ensure_rules_executable(source: &Path) {
    let rules = source.join("debian/rules");
    if !rules.is_file() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&rules)
            .unwrap_or_else(|error| panic!("stat {}: {error}", rules.display()))
            .permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(&rules, permissions)
            .unwrap_or_else(|error| panic!("chmod {}: {error}", rules.display()));
    }
}

fn dpkg_build_binary(source: &Path) {
    ensure_rules_executable(source);
    let output = Command::new("dpkg-buildpackage")
        .current_dir(source)
        .args(["-b", "-us", "-uc", "-d"])
        .env("DEB_BUILD_OPTIONS", "nocheck noautodbgsym")
        .stdin(Stdio::null())
        .output()
        .expect("spawning dpkg-buildpackage -b");
    assert!(
        output.status.success(),
        "{}: dpkg-buildpackage -b failed\nstdout:\n{}\nstderr:\n{}",
        source.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assemble_binary_packages(recipe: &Path, work: &Path) -> Vec<PathBuf> {
    if recipe_is_deb_skeleton(recipe) {
        let source = work.join("src");
        let build = work.join("build");
        copy_tree(recipe, &source);
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
            "{}: make failed\nstdout:\n{}\nstderr:\n{}",
            recipe.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return vec![find_binary_package(&build)];
    }

    let source = work.join("src");
    copy_package_tree(recipe, &source);
    dpkg_build_binary(&source);
    let packages = find_binary_packages(work);
    assert!(
        !packages.is_empty(),
        "{}: dpkg-buildpackage produced no Binary package in {}",
        recipe.display(),
        work.display()
    );
    packages
}

pub fn assert_binary_package_parity(recipe_rel: &str) {
    let recipe = parity_suite_root().join(recipe_rel);
    let catalogued = catalogued_binary_package_tags();
    let expected = catalogued_binary_package_hints(&recipe, &catalogued);
    assert!(
        !expected.is_empty(),
        "{recipe_rel}: listed Parity recipe has no catalogued Binary-package hints"
    );

    let work = tempfile::tempdir().expect("temp recipe workdir");
    let subjects = assemble_binary_packages(&recipe, work.path());

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

    let mut actual = Vec::new();
    for subject in &subjects {
        let (_status, stdout, stderr) = run_lint_subject(subject, &extra_args);
        assert!(
            stderr.is_empty() || !stdout.is_empty(),
            "{recipe_rel}: lint {} failed with empty stdout\n{stderr}",
            subject.display()
        );
        actual.extend(
            stdout
                .lines()
                .filter_map(parse_diagnostic_line)
                .filter(|(processable_type, hint)| {
                    catalogued.contains(&hint.tag) && applies_to_binary_package(processable_type)
                })
                .map(|(_, hint)| hint),
        );
    }
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (binary package)"
    );
}

fn copy_package_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap_or_else(|error| panic!("creating {}: {error}", dst.display()));
    for entry in
        fs::read_dir(src).unwrap_or_else(|error| panic!("reading {}: {error}", src.display()))
    {
        let entry = entry.expect("directory entry");
        let name = entry.file_name();
        if name == "ORIGIN" || name == "eval" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            copy_package_tree(&from, &to);
        } else if file_type.is_file() {
            fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!("copying {} to {}: {error}", from.display(), to.display())
            });
        }
    }
}

pub fn missing_upstream_tarball(stderr: &str) -> Option<String> {
    const MARKER: &str = "no upstream tarball found at ";
    for line in stderr.lines() {
        let Some((_, rest)) = line.split_once(MARKER) else {
            continue;
        };
        let token = rest.split_whitespace().next()?;
        let basename = Path::new(token).file_name()?.to_str()?;
        return Some(expand_orig_compression(basename));
    }
    None
}

fn expand_orig_compression(token: &str) -> String {
    if let Some((prefix, suffix)) = token.split_once(".{")
        && prefix.ends_with(".tar")
        && suffix.contains("gz")
    {
        return format!("{prefix}.gz");
    }
    token.to_string()
}

fn is_component_orig(name: &str) -> bool {
    name.contains(".orig-")
}

fn dpkg_source_build(work: &Path) -> (bool, String, String) {
    let output = Command::new("dpkg-source")
        .current_dir(work)
        .args(["-b", "src"])
        .stdin(Stdio::null())
        .output()
        .expect("spawning dpkg-source -b");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn pack_main_orig(tree: &Path, orig: &Path) {
    let output = Command::new("tar")
        .current_dir(tree)
        .args(["--exclude=./debian", "--exclude=debian", "-czf"])
        .arg(orig)
        .arg(".")
        .output()
        .expect("spawning tar for orig");
    assert!(
        output.status.success(),
        "packing {} failed\nstdout:\n{}\nstderr:\n{}",
        orig.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn find_dsc(dir: &Path) -> PathBuf {
    let mut found = Vec::new();
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_file() && path.extension().and_then(OsStr::to_str) == Some("dsc") {
            found.push(path);
        }
    }
    assert_eq!(
        found.len(),
        1,
        "expected one .dsc in {}, found {found:?}",
        dir.display()
    );
    found.remove(0)
}

fn assemble_source_package(recipe: &Path, work: &Path) -> PathBuf {
    let source = work.join("src");
    copy_package_tree(recipe, &source);
    let (ok, stdout, stderr) = dpkg_source_build(work);
    if !ok {
        let Some(name) = missing_upstream_tarball(&stderr) else {
            panic!("{recipe:?}: dpkg-source -b failed\n{stdout}\n{stderr}");
        };
        assert!(
            !is_component_orig(&name),
            "{recipe:?}: component orig {name} is not synthesized"
        );
        pack_main_orig(&source, &work.join(&name));
        let (ok, stdout, stderr) = dpkg_source_build(work);
        assert!(
            ok,
            "{recipe:?}: dpkg-source -b failed after orig {name}\n{stdout}\n{stderr}"
        );
    }
    find_dsc(work)
}

pub fn assert_source_package_parity(recipe_rel: &str) {
    let recipe = parity_suite_root().join(recipe_rel);
    let tags = catalogued_source_package_tags();
    let expected = catalogued_source_package_hints(&recipe, &tags);
    assert!(
        !expected.is_empty(),
        "{recipe_rel}: listed Parity recipe has no catalogued Source-package hints"
    );

    let work = tempfile::tempdir().expect("temp recipe workdir");
    let subject = assemble_source_package(&recipe, work.path());

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
            applies_to_source_package(processable_type, &hint.tag, &tags)
        })
        .map(|(_, hint)| hint)
        .collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (source package)"
    );
}
