//! Shared helpers for `debmagic lint` integration tests.

#![allow(dead_code)]

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::OnceLock,
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

/// `debmagic lint` of several artifacts at once.
pub fn run_lint_subjects(subjects: &[&Path], extra_args: &[&str]) -> (ExitStatus, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_debmagic"))
        .arg("lint")
        .args(subjects)
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
/// How many Rules a lint of a Source tree runs, read from the Rule sources
/// (one `declare_rule!` per file, ADR 0027) instead of a hand-kept number:
/// `default` is every non-experimental, default-selected Rule that implements
/// `SourceRule`; `ln` is every LN Rule that implements it (what `--select LN`
/// runs, ADR 0014).
pub struct SourceRuleCounts {
    pub default: usize,
    pub ln: usize,
}

pub fn source_rule_counts() -> SourceRuleCounts {
    let mut counts = SourceRuleCounts { default: 0, ln: 0 };
    let mut pending = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lint/rules")];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("rules directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains("impl SourceRule for") || !text.contains("declare_rule!") {
                continue;
            }
            let has = |line: &str| text.lines().any(|candidate| candidate.trim() == line);
            if has("default_selected = true,") && has("experimental = false,") {
                counts.default += 1;
            }
            if text
                .lines()
                .any(|line| line.trim().starts_with("code = \"LN"))
            {
                counts.ln += 1;
            }
        }
    }
    counts
}

/// Tags of the LN Rules a Source-tree lint can show: Rules of the Source and
/// Binary-package kinds. A Rule that inspects only the artifacts of a Group
/// (a Changes file, a Buildinfo file, a whole Group) is not one of them.
pub fn catalogued_ln_tags() -> HashSet<String> {
    catalogued_tags_with_impl(&["impl SourceRule for", "impl BinaryPackageRule for"])
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
    let extra = undo_double_encoding(&strip_location_pointer(fields.next().unwrap_or("").trim()));
    Some((
        processable_type.to_string(),
        ParityHint {
            tag: tag.to_string(),
            extra,
        },
    ))
}

/// lintian 2.144 prints a pointer path that is valid non-ASCII UTF-8 with
/// every byte read as Latin-1 (`鳥` shows as `é³¥`). Read such text back as the
/// UTF-8 it came from; text whose bytes are not UTF-8 (a Latin-1 file name)
/// stays as it is.
fn undo_double_encoding(text: &str) -> String {
    if text.is_ascii() {
        return text.to_string();
    }
    let bytes: Option<Vec<u8>> = text
        .chars()
        .map(|character| u8::try_from(u32::from(character)).ok())
        .collect();
    bytes
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| text.to_string())
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

/// `debhelper-compat-missing` keeps `debian/compat` so the package builds, and
/// `debian/source/options` tar-ignores it. The `(source)` hints
/// `package-uses-deprecated-debhelper-compat-version` and
/// `debhelper-base-compat-level-is-missing` are from the packed source, where
/// the file is gone. The filled tree still has compat 13, so Source-tree
/// Parity must not expect them. Source-package Parity packs with `dpkg-source`
/// and still expects them. Hints that need an orig a Source tree lacks are
/// dropped the same way. Kept in step with
/// `source_tree_hint_matches_filled_tree` in `scripts/import_lintian_parity.py`.
fn source_tree_hint_matches_filled_tree(recipe: &Path, hint: &ParityHint) -> bool {
    // `no-debian-changes` compares Patched files with Orig files. A Source tree
    // has no orig, so only the Source package can report it.
    if hint.tag == "no-debian-changes" {
        return false;
    }
    // `add-component-copyright` reads the orig tarball names of the `.dsc`. A
    // Source tree has none, so only the Source package can report it.
    if hint.tag == "add-component-copyright" {
        return false;
    }
    !(matches!(
        hint.tag.as_str(),
        "package-uses-deprecated-debhelper-compat-version"
            | "debhelper-base-compat-level-is-missing"
    ) && recipe.ends_with("checks/debhelper/debhelper-compat-missing"))
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
    catalogued_tags_with_impl(&["impl BinaryPackageRule for"])
}

/// Tags of LN Rules that implement `SourceRule`.
pub fn catalogued_source_package_tags() -> HashSet<String> {
    catalogued_tags_with_impl(&["impl SourceRule for"])
}

/// The Rule kinds `catalogued_tags_with_impl` can filter on.
const RULE_IMPL_NEEDLES: &[&str] = &[
    "impl SourceRule for",
    "impl BinaryPackageRule for",
    "impl ChangesRule for",
    "impl BuildinfoRule for",
    "impl GroupRule for",
];

/// One Rule source file of `src/lint/rules/lintian/`: which Rule kinds it
/// implements and the Tags it declares.
struct CataloguedRuleFile {
    impls: Vec<&'static str>,
    tags: Vec<String>,
}

/// The catalogue scan, done once per test process: it reads every Rule file
/// under `src/lint/rules/lintian/`, and each test needs several views of it.
fn catalogue_scan() -> &'static [CataloguedRuleFile] {
    static SCAN: OnceLock<Vec<CataloguedRuleFile>> = OnceLock::new();
    SCAN.get_or_init(|| {
        fn walk(dir: &Path, files: &mut Vec<CataloguedRuleFile>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let impls: Vec<&'static str> = RULE_IMPL_NEEDLES
                    .iter()
                    .copied()
                    .filter(|needle| text.contains(needle))
                    .collect();
                if impls.is_empty() {
                    continue;
                }
                let tags = text
                    .lines()
                    .filter_map(|line| {
                        line.trim()
                            .strip_prefix("tag = \"")?
                            .strip_suffix("\",")
                            .map(str::to_string)
                    })
                    .collect();
                files.push(CataloguedRuleFile { impls, tags });
            }
        }
        let mut files = Vec::new();
        walk(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lint/rules/lintian"),
            &mut files,
        );
        files
    })
}

fn catalogued_tags_with_impl(needles: &[&str]) -> HashSet<String> {
    debug_assert!(
        needles
            .iter()
            .all(|needle| RULE_IMPL_NEEDLES.contains(needle))
    );
    catalogue_scan()
        .iter()
        .filter(|file| file.impls.iter().any(|kind| needles.contains(kind)))
        .flat_map(|file| file.tags.iter().cloned())
        .collect()
}

/// A recipe's `eval/hints` or `eval/desc`, lossily decoded. A test reads the
/// same file several times (expected hints, `debian-adds-` pointers, Tag
/// prefixes), so the text is kept for the rest of the process.
fn recipe_eval_text(path: &Path) -> Option<std::sync::Arc<str>> {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    static TEXTS: OnceLock<Mutex<HashMap<PathBuf, Option<Arc<str>>>>> = OnceLock::new();
    let texts = TEXTS.get_or_init(Default::default);
    if let Some(text) = texts.lock().expect("eval text cache").get(path) {
        return text.clone();
    }
    let text = fs::read(path)
        .ok()
        .map(|bytes| Arc::<str>::from(String::from_utf8_lossy(&bytes).as_ref()));
    texts
        .lock()
        .expect("eval text cache")
        .insert(path.to_path_buf(), text.clone());
    text
}

fn load_hints(path: &Path) -> Vec<(String, ParityHint)> {
    let Some(text) = recipe_eval_text(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_universal_hint_line).collect()
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
            catalogued.contains(&hint.tag)
                && applies_to_source_tree(processable_type, &hint.extra)
                && source_tree_hint_matches_filled_tree(recipe, hint)
        })
        .map(|(_, hint)| hint)
        .collect();
    correct_stale_expected_hints(recipe, &mut expected);
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
    if let Some(reason) = skipped_recipe_reason(recipe_rel).or_else(|| {
        RECIPES_SKIPPED_FOR_SOURCE_TREE
            .iter()
            .find(|(name, _)| *name == recipe_rel)
            .map(|(_, reason)| *reason)
    }) {
        eprintln!("{recipe_rel}: skipped, {reason}");
        return;
    }
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

    // The recipe directory also holds ORIGIN and eval/, which are not part of
    // the Source tree lintian saw; lint a copy without them.
    let work = tempfile::tempdir().expect("temp recipe workdir");
    let source = work.path().join("src");
    copy_package_tree(&recipe, &source);
    apply_overrides_to_control(&source, &dpkg_overrides(&recipe));

    let (_status, stdout, stderr) = run_lint(&source, &extra_args);
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
    rewrite_changelog_last_used(recipe_rel, &mut actual);
    rewrite_current_policy_release_date(recipe_rel, &mut actual);
    rewrite_current_policy_version(recipe_rel, &mut actual);
    drop_hints_for_unvendorable_files(recipe_rel, &mut actual);
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
    correct_stale_expected_hints(recipe, &mut expected);
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
        .map(|mut hint| {
            hint.extra = expand_host_placeholders(&hint.extra);
            hint
        })
        .collect();
    expected.sort();
    expected
}

/// Lintian's recipes print the host's Perl vendor architecture directory as
/// `usr/lib/ma-dir/perl/version` (the recipe's `debian/rules` installs into
/// `$Config{vendorarch}`); expand it to what this host builds.
fn expand_host_placeholders(extra: &str) -> String {
    const PLACEHOLDER: &str = "usr/lib/ma-dir/perl/version";
    if !extra.contains(PLACEHOLDER) {
        return extra.to_string();
    }
    static VENDORARCH: OnceLock<String> = OnceLock::new();
    let vendorarch = VENDORARCH.get_or_init(|| {
        let output = Command::new("perl")
            .args(["-MConfig", "-wE", "say substr($Config{vendorarch},1)"])
            .output()
            .expect("run perl to find the vendor architecture directory");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    });
    extra.replace(PLACEHOLDER, vendorarch)
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

/// Git cannot track a directory named `.git`, so the importer vendors the
/// recipes' `.git/` cruft under this name; copies of a recipe restore it.
const VENDORED_GIT_DIR: &str = ".git.vendored";

/// The name a recipe Entry has in the tree lintian saw.
fn unvendored_name(name: std::ffi::OsString) -> std::ffi::OsString {
    if name == VENDORED_GIT_DIR {
        ".git".into()
    } else {
        name
    }
}

/// The importer keeps the parts of a recipe's `build-spec` a filled tree cannot
/// show under this name (`dpkg-overrides`, `pre-upstream.in`). It is not part
/// of the tree lintian saw, so copies of a recipe leave it out.
const VENDORED_BUILD_SPEC: &str = "build-spec";

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap_or_else(|error| panic!("creating {}: {error}", dst.display()));
    for entry in
        fs::read_dir(src).unwrap_or_else(|error| panic!("reading {}: {error}", src.display()))
    {
        let entry = entry.expect("directory entry");
        let from = entry.path();
        let to = dst.join(unvendored_name(entry.file_name()));
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            copy_tree(&from, &to);
        } else if file_type.is_file() {
            fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!("copying {} to {}: {error}", from.display(), to.display())
            });
            // Git keeps only the executable bit, so a checkout under a
            // group-writable umask (002) leaves 0664 and 0775 files that
            // lintian's recipes never had. Drop group and other write.
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = fs::metadata(&to).expect("stat copied file").permissions();
                permissions.set_mode(permissions.mode() & !0o022);
                fs::set_permissions(&to, permissions).expect("chmod copied file");
            }
        } else if file_type.is_symlink() {
            let target = fs::read_link(&from)
                .unwrap_or_else(|error| panic!("reading link {}: {error}", from.display()));
            std::os::unix::fs::symlink(&target, &to).unwrap_or_else(|error| {
                panic!("linking {} to {}: {error}", to.display(), target.display())
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

/// The `dpkg-deb` wrapper [`dpkg_deb_nocheck_path`] writes. Part of the build
/// cache key: a changed wrapper can change a built package.
const DPKG_DEB_NOCHECK_WRAPPER: &str = r#"#!/usr/bin/python3
import os, sys

real = "/usr/bin/dpkg-deb"
args = sys.argv[1:]
try:
    idx = next(i for i, arg in enumerate(args) if arg in ("--build", "-b"))
except StopIteration:
    os.execv(real, [real, *args])

# `dpkg --build` execs `dpkg-deb --build -- <dir> <dest>`. `--` ends options.
positionals = []
tail_opts = []
for arg in args[idx + 1 :]:
    if arg == "--" and not positionals and not tail_opts:
        continue
    if arg.startswith("-") and not positionals:
        tail_opts.append(arg)
        continue
    if arg.startswith("-"):
        tail_opts.append(arg)
        continue
    positionals.append(arg)

def control_fields(directory):
    control = os.path.join(directory, "DEBIAN", "control")
    fields = {}
    with open(control, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line.strip() or line[:1] in " \t" or ":" not in line:
                continue
            key, value = line.split(":", 1)
            fields.setdefault(key, value.strip())
    return fields

rewritten = False
if len(positionals) >= 2 and os.path.isdir(positionals[1]):
    try:
        fields = control_fields(positionals[0])
        version = fields["Version"]
        if ":" in version:
            version = version.split(":", 1)[1]
        extension = "udeb" if fields.get("Package-Type") == "udeb" else "deb"
        filename = f"{fields['Package']}_{version}_{fields['Architecture']}.{extension}"
        positionals[1] = os.path.join(positionals[1], filename)
        rewritten = True
    except (OSError, KeyError):
        rewritten = False

file_dest = len(positionals) >= 2 and not os.path.isdir(positionals[1])
command = [real]
if rewritten or file_dest:
    command.append("--nocheck")
command.extend(args[: idx + 1])
command.extend(positionals)
command.extend(tail_opts)
os.execv(real, command)
"#;

/// Prepend a `dpkg-deb` that passes `--nocheck`.
///
/// Lintian recipes build packages current `dpkg-deb` rejects, such as a
/// `config` script mode `0644`. `--nocheck` refuses a directory target, so a
/// directory destination is rewritten to the `.deb` path `dpkg-deb` would
/// have chosen (epoch omitted from the filename). Archive bytes of a package
/// that already passed the checks stay the same.
fn dpkg_deb_nocheck_path(source: &Path) -> PathBuf {
    let bin = source
        .parent()
        .expect("recipe source lives in a work directory")
        .join("bin");
    fs::create_dir_all(&bin).expect("creating dpkg-deb wrapper dir");
    let wrapper = bin.join("dpkg-deb");
    fs::write(&wrapper, DPKG_DEB_NOCHECK_WRAPPER).expect("writing dpkg-deb wrapper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&wrapper)
            .expect("stat dpkg-deb wrapper")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&wrapper, permissions).expect("chmod dpkg-deb wrapper");
    }
    bin
}

/// `pkg-js-tools` provides `Debian::Debhelper::Sequence::nodejs`. Without it,
/// `dh --with nodejs` cannot even clean the source.
fn nodejs_sequence_missing() -> bool {
    Command::new("perl")
        .args(["-MDebian::Debhelper::Sequence::nodejs", "-e", "1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| !status.success())
}

fn rules_use_nodejs(source: &Path) -> bool {
    let rules = fs::read_to_string(source.join("debian/rules")).unwrap_or_default();
    let control = fs::read_to_string(source.join("debian/control")).unwrap_or_default();
    rules.contains("--with nodejs")
        || rules.contains("dh-sequence-nodejs")
        || control.contains("dh-sequence-nodejs")
}

// --- Build cache ---------------------------------------------------------------------------------
//
// Recipes whose copied trees are identical build the same product, so a tree is
// built once per test process. The key is a SHA-256 over the copied tree and
// everything else that changes the product (see `BuildKey`); a recipe's
// `eval/hints` and `eval/desc` are not in it, because they are what the test
// compares against, not what is built.

/// Bumped when the key layout or what is stored changes.
const BUILD_CACHE_FORMAT: &str = "3";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Builds the key of one build: every input is added as a named,
/// length-prefixed field, so two different inputs never hash alike.
struct BuildKey(sha2::Sha256);

impl BuildKey {
    fn new(kind: &str) -> Self {
        let mut key = Self(sha2::Sha256::default());
        key.field("format", BUILD_CACHE_FORMAT.as_bytes());
        key.field("kind", kind.as_bytes());
        key.field("environment", build_environment_fingerprint().as_bytes());
        key
    }

    fn field(&mut self, name: &str, value: &[u8]) {
        use sha2::Digest;
        for part in [name.as_bytes(), value] {
            self.0.update((part.len() as u64).to_le_bytes());
            self.0.update(part);
        }
    }

    /// The content of a copied tree: names, kinds, modes, file contents and
    /// link targets, in name order. Not modification times: the copy gets new
    /// ones, and packages clamp them to `SOURCE_DATE_EPOCH`.
    fn tree(&mut self, dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
            .map(|entry| entry.expect("directory entry"))
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        self.field("entries", &(entries.len() as u64).to_le_bytes());
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().expect("file type");
            self.field("name", entry.file_name().as_encoded_bytes());
            let mode = fs::symlink_metadata(&path)
                .unwrap_or_else(|error| panic!("stat {}: {error}", path.display()))
                .permissions()
                .mode()
                & 0o7777;
            if file_type.is_dir() {
                self.field("dir", &mode.to_le_bytes());
                self.tree(&path);
            } else if file_type.is_symlink() {
                let target = fs::read_link(&path)
                    .unwrap_or_else(|error| panic!("reading link {}: {error}", path.display()));
                self.field("link", target.as_os_str().as_encoded_bytes());
            } else {
                let bytes = fs::read(&path)
                    .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
                self.field("file", &mode.to_le_bytes());
                self.field("content", &bytes);
            }
        }
    }

    /// Whether `dpkg_buildpackage_command` puts the `dh --with nodejs` stub on
    /// the path for this tree, and then what the stub is.
    fn nodejs_stub(&mut self, source: &Path) {
        let use_stub = rules_use_nodejs(source) && nodejs_sequence_missing();
        self.field("nodejs-stub", &[u8::from(use_stub)]);
        if use_stub {
            self.tree(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/dh_sequence_nodejs"));
        }
    }

    fn finish(self) -> String {
        use sha2::Digest;
        hex(&self.0.finalize())
    }
}

/// What the host contributes to every build, hashed once per process: the host
/// architecture, the user (a `dpkg-deb` without fakeroot packs the owner and
/// group of the builder), the environment variables the build tools read, and
/// the installed packages (`/var/lib/dpkg/status` stands for the toolchain:
/// dpkg, debhelper, compilers). `PATH` and the directory the tests run in are
/// left out on purpose: they differ between shells and do not change a
/// package.
fn build_environment_fingerprint() -> &'static str {
    static FINGERPRINT: OnceLock<String> = OnceLock::new();
    FINGERPRINT.get_or_init(|| {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::default();
        let mut field = |name: &str, value: &[u8]| {
            for part in [name.as_bytes(), value] {
                hasher.update((part.len() as u64).to_le_bytes());
                hasher.update(part);
            }
        };
        field("host-arch", host_architecture().unwrap_or("?").as_bytes());
        // SAFETY: `getuid` and `getgid` take no arguments and cannot fail.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        field("uid", &uid.to_le_bytes());
        field("gid", &gid.to_le_bytes());
        let mut variables: Vec<(String, std::ffi::OsString)> = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value)))
            .filter(|(name, _)| {
                ["DEB_", "DH_", "DPKG_", "LC_"]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
                    || matches!(
                        name.as_str(),
                        "LANG"
                            | "LANGUAGE"
                            | "TZ"
                            | "CC"
                            | "CXX"
                            | "CPP"
                            | "CFLAGS"
                            | "CPPFLAGS"
                            | "CXXFLAGS"
                            | "LDFLAGS"
                            | "FFLAGS"
                            | "MAKEFLAGS"
                            | "MFLAGS"
                            | "SOURCE_DATE_EPOCH"
                            | "PERL5LIB"
                    )
            })
            .collect();
        variables.sort();
        for (name, value) in &variables {
            field(&format!("env:{name}"), value.as_encoded_bytes());
        }
        let status = fs::read("/var/lib/dpkg/status").unwrap_or_default();
        field("dpkg-status", sha2::Sha256::digest(&status).as_slice());
        hex(&hasher.finalize())
    })
}

/// The build products of one key: file name and bytes.
type BuiltFiles = Vec<(String, Vec<u8>)>;

type BuildSlot = std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<BuiltFiles>>>>;

/// `DEBMAGIC_PARITY_NO_CACHE=1` turns the build cache off, in memory and on
/// disk: every recipe builds its own tree.
fn build_cache_bypassed() -> bool {
    static BYPASSED: OnceLock<bool> = OnceLock::new();
    *BYPASSED.get_or_init(|| {
        std::env::var_os("DEBMAGIC_PARITY_NO_CACHE")
            .is_some_and(|value| !value.is_empty() && value != "0")
    })
}

/// `<target dir>/lint-parity-cache`, next to the `debmagic` binary the tests
/// run (`target/debug/debmagic`), so `cargo clean` clears it.
fn build_cache_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let target = Path::new(env!("CARGO_BIN_EXE_debmagic"))
            .ancestors()
            .nth(2)?;
        let dir = target.join("lint-parity-cache");
        fs::create_dir_all(&dir).ok()?;
        // A killed run can leave its half-written entry behind under a
        // `.tmp-` name; it is never read, only removed once it is old.
        let stale = std::time::Duration::from_secs(3600);
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let leftover = entry.file_name().to_string_lossy().starts_with(".tmp-")
                && entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > stale);
            if leftover {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        Some(dir)
    })
    .as_deref()
}

const BUILD_CACHE_MANIFEST: &str = "manifest";

fn digest_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex(&sha2::Sha256::digest(bytes))
}

/// The products stored under `key`, or `None` when there is no entry or it is
/// damaged (a missing or altered file, a manifest of another key). A damaged
/// entry is removed so the rebuild can take its place.
fn read_cache_entry(cache: &Path, key: &str) -> Option<BuiltFiles> {
    let entry = cache.join(key);
    if !entry.exists() {
        return None;
    }
    let read = || -> Option<BuiltFiles> {
        let manifest = fs::read_to_string(entry.join(BUILD_CACHE_MANIFEST)).ok()?;
        let mut lines = manifest.lines();
        let header = lines.next()?;
        (header == format!("debmagic-parity-cache {BUILD_CACHE_FORMAT} {key}")).then_some(())?;
        let mut files = Vec::new();
        for line in lines {
            let mut fields = line.splitn(3, '\t');
            let (digest, size, name) = (fields.next()?, fields.next()?, fields.next()?);
            let bytes = fs::read(entry.join(name)).ok()?;
            (bytes.len().to_string() == size && digest_hex(&bytes) == digest).then_some(())?;
            files.push((name.to_string(), bytes));
        }
        Some(files)
    };
    let files = read();
    if files.is_none() {
        // Move it away first: a rename is atomic, so a process that is
        // reading the same entry sees it whole or not at all.
        let trash = cache.join(format!(
            ".tmp-bad-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        if fs::rename(&entry, &trash).is_ok() {
            let _ = fs::remove_dir_all(&trash);
        }
    }
    files
}

/// Store `files` under `key`: written in full under a temporary name, then
/// renamed into place, so a reader (or a run killed halfway) never meets a
/// partial entry. Returns whether the entry exists afterwards; losing the
/// rename to another process that stored the same key counts as success.
fn write_cache_entry(cache: &Path, key: &str, files: &BuiltFiles) -> bool {
    let tmp = cache.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let store = || -> std::io::Result<()> {
        fs::create_dir(&tmp)?;
        let mut manifest = format!("debmagic-parity-cache {BUILD_CACHE_FORMAT} {key}\n");
        for (name, bytes) in files {
            fs::write(tmp.join(name), bytes)?;
            manifest.push_str(&format!("{}\t{}\t{name}\n", digest_hex(bytes), bytes.len()));
        }
        fs::write(tmp.join(BUILD_CACHE_MANIFEST), manifest)?;
        fs::rename(&tmp, cache.join(key))
    };
    let stored = store().is_ok() || cache.join(key).join(BUILD_CACHE_MANIFEST).is_file();
    let _ = fs::remove_dir_all(&tmp);
    stored
}

/// Put `files` into `dest`, each written under a temporary name and renamed.
fn write_products(dest: &Path, files: &BuiltFiles) -> Vec<PathBuf> {
    fs::create_dir_all(dest).unwrap_or_else(|error| panic!("creating {}: {error}", dest.display()));
    files
        .iter()
        .map(|(name, bytes)| {
            let path = dest.join(name);
            let tmp = dest.join(format!(".{name}.tmp"));
            fs::write(&tmp, bytes)
                .and_then(|()| fs::rename(&tmp, &path))
                .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
            path
        })
        .collect()
}

/// The files `build` produces in `dest`, built once per key.
///
/// The products are stored under `target/lint-parity-cache/<key>/` (build
/// products only, never lint results), so a later run of the suite builds
/// nothing it built before. Within a run, a test that needs a key somebody
/// else is building waits for that build (one lock per key). A test that finds
/// the key stored writes the stored files into its own `dest` and does not run
/// `build`; a missing or damaged entry is a rebuild.
///
/// A build that panics stores nothing: the panic is the failure of the recipe
/// that ran it, and the next recipe with the same tree builds it itself, so
/// one recipe's failure (or a flaky one) never fails the recipes that share
/// its tree, and a failure is never cached on disk.
///
/// `build` returns the paths of the products, all directly in `dest`. With
/// `DEBMAGIC_PARITY_NO_CACHE=1` it always runs.
fn build_once(key: String, dest: &Path, build: impl FnOnce() -> Vec<PathBuf>) -> Vec<PathBuf> {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, PoisonError},
    };
    if build_cache_bypassed() {
        return build();
    }
    static SLOTS: OnceLock<Mutex<HashMap<String, BuildSlot>>> = OnceLock::new();
    let slot = SLOTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(key.clone())
        .or_default()
        .clone();
    // A poisoned lock holds `None`: the build that panicked stored nothing.
    let mut in_memory = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(files) = in_memory.as_ref() {
        return write_products(dest, files);
    }
    let cache = build_cache_dir();
    if let Some(files) = cache.and_then(|cache| read_cache_entry(cache, &key)) {
        return write_products(dest, &files);
    }
    let paths = build();
    let files: BuiltFiles = paths
        .iter()
        .map(|path| {
            assert_eq!(
                path.parent(),
                Some(dest),
                "{} is not in {}",
                path.display(),
                dest.display()
            );
            let bytes = fs::read(path)
                .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
            let name = path
                .file_name()
                .and_then(OsStr::to_str)
                .expect("product name is UTF-8");
            (name.to_string(), bytes)
        })
        .collect();
    // Without a usable cache directory the products stay in memory.
    if !cache.is_some_and(|cache| write_cache_entry(cache, &key, &files)) {
        *in_memory = Some(Arc::new(files));
    }
    paths
}

/// `dpkg-buildpackage` in `source`, with the host fixes recipes need: a
/// `dpkg-deb` that passes `--nocheck`, a stub for `dh --with nodejs` when
/// `pkg-js-tools` is missing, and umask 022 (lintian's recipes are built
/// under it). The caller adds the arguments and runs it.
fn dpkg_buildpackage_command(source: &Path, build_options: &str) -> Command {
    let stub = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/dh_sequence_nodejs");
    let use_nodejs_stub = rules_use_nodejs(source) && nodejs_sequence_missing();
    let mut path = dpkg_deb_nocheck_path(source).into_os_string();
    path.push(":");
    if use_nodejs_stub {
        path.push(stub.join("bin").as_os_str());
        path.push(":");
    }
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let mut command = Command::new("dpkg-buildpackage");
    command
        .current_dir(source)
        .env("DEB_BUILD_OPTIONS", build_options)
        .env("PATH", &path)
        .stdin(Stdio::null());
    if use_nodejs_stub {
        let mut perl5lib = stub.into_os_string();
        if let Some(existing) = std::env::var_os("PERL5LIB") {
            perl5lib.push(":");
            perl5lib.push(existing);
        }
        command.env("PERL5LIB", perl5lib);
    }
    // lintian's recipes are built under umask 022; a group-writable host umask
    // (002) would make the compiler emit 0775 libraries.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: `umask` is async-signal-safe and touches no Rust state.
        unsafe {
            command.pre_exec(|| {
                libc::umask(0o022);
                Ok(())
            });
        }
    }
    command
}

/// `DEB_BUILD_OPTIONS` and arguments of the Binary-package build. Part of the
/// build cache key.
const BINARY_BUILD_OPTIONS: &str = "nocheck noautodbgsym";
const BINARY_BUILD_ARGS: &[&str] = &["-b", "-us", "-uc", "-d"];

fn dpkg_build_binary(source: &Path) {
    ensure_rules_executable(source);
    let mut command = dpkg_buildpackage_command(source, BINARY_BUILD_OPTIONS);
    command.args(BINARY_BUILD_ARGS);
    let output = command.output().expect("spawning dpkg-buildpackage -b");
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
        // The whole copy is hashed, `ORIGIN` and `eval/` included: the Makefile
        // runs in it and may read any of it.
        let mut key = BuildKey::new("deb-skeleton");
        key.tree(&source);
        return build_once(key.finish(), &build, || {
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
            vec![find_binary_package(&build)]
        });
    }

    let source = work.join("src");
    copy_package_tree(recipe, &source);
    ensure_rules_executable(&source);
    let mut key = BuildKey::new("dpkg-buildpackage-binary");
    key.tree(&source);
    key.field("build-options", BINARY_BUILD_OPTIONS.as_bytes());
    key.field("build-args", BINARY_BUILD_ARGS.join(" ").as_bytes());
    key.field("dpkg-deb-wrapper", DPKG_DEB_NOCHECK_WRAPPER.as_bytes());
    key.nodejs_stub(&source);
    build_once(key.finish(), work, || {
        dpkg_build_binary(&source);
        let packages = find_binary_packages(work);
        assert!(
            !packages.is_empty(),
            "{}: dpkg-buildpackage produced no Binary package in {}",
            recipe.display(),
            work.display()
        );
        packages
    })
}

/// Binary-package recipes this host's toolchain cannot build, with why. Their
/// Rules have native tests; Parity needs a host that can build the recipe.
const RECIPES_THE_HOST_CANNOT_BUILD: &[(&str, &str)] = &[
    (
        "checks/deb-format/deb-format-malformed",
        "the recipe's `tar-data` script runs `tar -Z`, which needs the `compress` tool, and the host does not have it",
    ),
    (
        "checks/languages/r/site-library/lacks-depends-on-r-api",
        "the recipe builds with `--buildsystem=R`, and the host has no `Debian::Debhelper::Buildsystem::R` (the `dh-r` package)",
    ),
    (
        "checks/debian/debconf/debconf-config-not-executable",
        "the recipe's `control-file-has-bad-permissions config 0644 != 0755` hint names the file, which lintian 2.144.0's ControlFiles.pm does not emit (the control-files-bad recipe agrees with the code)",
    ),
    (
        "checks/debian/upstream/metadata/upstream-metadata-is-not-a-file",
        "the recipe's pre-build makes a FIFO at `debian/upstream/metadata`, which the vendored tree cannot hold",
    ),
    (
        "checks/binaries/obsolete/crypt/binaries-obsolete-des",
        "the host's libxcrypt has no `crypt.h` and no longer exports `setkey` and `encrypt`",
    ),
    (
        "checks/languages/java/java-class-format",
        "the recipe needs `javahelper` and a JDK (`javac`, `jar`), which this host lacks",
    ),
    (
        "checks/languages/java/java-classpath",
        "the recipe needs `javahelper` and a JDK (`javac`, `jar`), which this host lacks",
    ),
    (
        "checks/languages/java/java-jars",
        "the recipe needs `javahelper` and a JDK (`javac`, `jar`), which this host lacks",
    ),
    (
        "checks/debhelper/debhelper-compat-old",
        "the recipe's `--include-dir` vendor overrides `debhelper/compat-level` to deprecated=1 and recommended=100 for the lintian-test vendor; the harness applies only the Debian tables, so the expected level-8 hint cannot come out",
    ),
    (
        "checks/deb-format/deb-format-udeb-compression",
        "the recipe runs `dpkg-deb -Zlzma`, which dpkg 1.23 rejects",
    ),
    (
        "checks/binaries/prerequisites/numpy/binaries-missing-depends-on-numpy-abi",
        "the recipe's `setup.py` imports `setuptools` and `numpy`, and this host has neither for python3",
    ),
];

/// Recipes whose expected hints cannot show up in a Source tree, because the
/// tree is not the built product (a patch is not applied, say).
const RECIPES_SKIPPED_FOR_SOURCE_TREE: &[(&str, &str)] = &[
    (
        "checks/files/names/national-encoding-in-patch",
        "the Latin-1 file name comes from a patch that a Source tree does not apply",
    ),
    (
        "checks/archive/file/name/length/filename-length-really-really-really-really-long-package-name",
        "the hints name the orig and debian tarballs of the Files field of a .dsc, which a Source tree does not have",
    ),
    (
        "checks/fields/checksums/no-sha256-checksum",
        "the hint is about the built `.dsc`'s `Checksums-Sha256`, and a Source tree has no `.dsc`",
    ),
    (
        "checks/fields/multi-line/multiple-lines-in-maintainer-field",
        "the recipe's `post-build` hook breaks the `Maintainer` line of the built `.dsc`, and a Source tree has no `.dsc`",
    ),
];

/// The reason a recipe is skipped on this host, if it is.
fn skipped_recipe_reason(recipe_rel: &str) -> Option<&'static str> {
    RECIPES_THE_HOST_CANNOT_BUILD
        .iter()
        .find(|(name, _)| *name == recipe_rel)
        .map(|(_, reason)| *reason)
}

/// `dpkg-architecture --query=DEB_HOST_ARCH`, asked once per test process.
fn host_architecture() -> Option<&'static str> {
    static HOST: OnceLock<Option<String>> = OnceLock::new();
    HOST.get_or_init(|| {
        Command::new("dpkg-architecture")
            .arg("--query=DEB_HOST_ARCH")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    })
    .as_deref()
}

/// The recipe's `Test-Architectures` (`eval/desc`) when none of them is the
/// host architecture, as lintian's own test runner skips such a recipe: a
/// `.deb` for another architecture cannot be built here.
fn unmet_test_architectures(recipe: &Path) -> Option<String> {
    let desc = recipe_eval_text(&recipe.join("eval/desc"))?;
    let line = desc
        .lines()
        .find_map(|line| line.strip_prefix("Test-Architectures:"))?;
    let host = host_architecture()?;
    // `any-amd64` and the like are architecture wildcards; `-i` matches one
    // against a host architecture.
    let matches = |pattern: &str| {
        Command::new("dpkg-architecture")
            .args(["-a", host, "-i", pattern])
            .status()
            .is_ok_and(|status| status.success())
    };
    (!line.split_whitespace().any(matches)).then(|| line.trim().to_string())
}

pub fn assert_binary_package_parity(recipe_rel: &str) {
    let recipe = parity_suite_root().join(recipe_rel);
    if let Some((_, reason)) = RECIPES_THE_HOST_CANNOT_BUILD
        .iter()
        .find(|(name, _)| *name == recipe_rel)
    {
        eprintln!("{recipe_rel}: skipped, {reason}");
        return;
    }
    if let Some(architectures) = unmet_test_architectures(&recipe) {
        eprintln!("{recipe_rel}: skipped, the recipe is only built for {architectures}");
        return;
    }
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
    rewrite_font_package_list(recipe_rel, &mut actual);
    rewrite_incompatibility_statement(recipe_rel, &mut actual);
    rewrite_control_file_group(recipe_rel, &mut actual);
    rewrite_perl_core_version(recipe_rel, &mut actual);
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
        if name == "ORIGIN" || name == "eval" || name == VENDORED_BUILD_SPEC {
            continue;
        }
        let from = entry.path();
        let to = dst.join(unvendored_name(name));
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            copy_package_tree(&from, &to);
        } else if file_type.is_file() {
            fs::copy(&from, &to).unwrap_or_else(|error| {
                panic!("copying {} to {}: {error}", from.display(), to.display())
            });
        } else if file_type.is_symlink() {
            let target = fs::read_link(&from)
                .unwrap_or_else(|error| panic!("reading link {}: {error}", from.display()));
            std::os::unix::fs::symlink(&target, &to).unwrap_or_else(|error| {
                panic!("linking {} to {}: {error}", to.display(), target.display())
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

fn dpkg_source_build(
    work: &Path,
    keep_ignored: bool,
    overrides: &[String],
) -> (bool, String, String) {
    let mut command = Command::new("dpkg-source");
    command.current_dir(work).args(overrides).arg("-b");
    if keep_ignored {
        // lintian's upload builder passes these so a recipe's VCS cruft is
        // packed. The default ignore would drop it before a Rule can see it.
        command.args(["-iNEVER_MATCH_ANYTHING", "-INEVER_MATCH_ANYTHING"]);
    }
    command.arg("src").stdin(Stdio::null());
    let output = command.output().expect("spawning dpkg-source -b");
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

/// Pointers from `debian-adds-*` hint lines. The location is still in the
/// file; [`load_hints`] strips it.
fn debian_adds_pointers(recipe: &Path) -> Vec<String> {
    let Some(text) = recipe_eval_text(&recipe.join("eval/hints")) else {
        return Vec::new();
    };
    let mut pointers = Vec::new();
    for line in text.lines() {
        let Some((_, rest)) = line.split_once("): ") else {
            continue;
        };
        let Some(tag) = rest.split_whitespace().next() else {
            continue;
        };
        if !tag.starts_with("debian-adds-") {
            continue;
        }
        let Some((_, pointer)) = rest.rsplit_once(" [") else {
            continue;
        };
        let pointer = pointer.trim_end_matches(']').trim();
        if !pointer.is_empty() {
            pointers.push(pointer.to_string());
        }
    }
    pointers
}

/// Whether the recipe's `eval/hints` expect a Tag starting with `prefix`.
fn expects_tag_prefix(recipe: &Path, prefix: &str) -> bool {
    let Some(text) = recipe_eval_text(&recipe.join("eval/hints")) else {
        return false;
    };
    text.lines()
        .filter_map(|line| line.split_once("): "))
        .filter_map(|(_, rest)| rest.split_whitespace().next())
        .any(|tag| tag.starts_with(prefix))
}

fn outside_debian(pointer: &str) -> bool {
    let pointer = pointer.trim_start_matches("./");
    pointer != "debian" && !pointer.starts_with("debian/")
}

/// `<source>_<upstream>.orig.tar.gz` from `debian/changelog`.
fn main_orig_name(source: &Path) -> String {
    let changelog = source.join("debian/changelog");
    let package = changelog_field(&changelog, "Source");
    let version = changelog_field(&changelog, "Version");
    let no_epoch = version
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(&version);
    let upstream = no_epoch
        .rsplit_once('-')
        .map(|(upstream, _)| upstream)
        .unwrap_or(no_epoch);
    format!("{package}_{upstream}.orig.tar.gz")
}

fn changelog_field(changelog: &Path, field: &str) -> String {
    let output = Command::new("dpkg-parsechangelog")
        .args(["-l"])
        .arg(changelog)
        .args(["-S", field])
        .output()
        .unwrap_or_else(|error| panic!("spawning dpkg-parsechangelog: {error}"));
    assert!(
        output.status.success(),
        "dpkg-parsechangelog -S {field} failed for {}\n{}",
        changelog.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Orig without `debian/` and without root paths the hints call `debian-adds-`.
/// Those paths were created by the recipe `pre-build` after lintian packed
/// orig, so they belong in the diff.
fn pack_orig_without_added(tree: &Path, orig: &Path, added: &[String]) {
    let mut command = Command::new("tar");
    command
        .current_dir(tree)
        .args(["--exclude=./debian", "--exclude=debian"]);
    for pointer in added {
        let trimmed = pointer.trim_end_matches('/');
        command.arg(format!("--exclude=./{trimmed}"));
        command.arg(format!("--exclude={trimmed}"));
    }
    let output = command
        .args(["-czf"])
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

/// The `-DField=value` words of the recipe's vendored `build-spec/dpkg-overrides`,
/// split the way the shell behind lintian's Makefile splits them: on whitespace
/// outside double quotes, with the quotes removed.
fn dpkg_overrides(recipe: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(recipe.join(VENDORED_BUILD_SPEC).join("dpkg-overrides"))
    else {
        return Vec::new();
    };
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    for character in text.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            character => {
                word.push(character);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    for word in &words {
        assert!(
            word.strip_prefix("-D")
                .is_some_and(|rest| rest.contains('=')),
            "{recipe:?}: dpkg-overrides word {word:?} is not -DField=value"
        );
    }
    words
}

/// `dpkg-source -DField=value` sets a field of the `.dsc`, which a Source tree
/// does not have. The tree's stand-in is the Source paragraph of
/// `debian/control`, where the field is set the same way: it replaces an
/// existing one and otherwise goes at the end of the paragraph.
fn apply_overrides_to_control(source: &Path, overrides: &[String]) {
    if overrides.is_empty() {
        return;
    }
    let control = source.join("debian/control");
    let mut text = fs::read_to_string(&control)
        .unwrap_or_else(|error| panic!("reading {}: {error}", control.display()));
    for word in overrides {
        let (field, value) = word
            .strip_prefix("-D")
            .and_then(|rest| rest.split_once('='))
            .expect("dpkg_overrides checked the form");
        text = set_source_paragraph_field(&text, field, value);
    }
    fs::write(&control, text)
        .unwrap_or_else(|error| panic!("writing {}: {error}", control.display()));
}

fn set_source_paragraph_field(control: &str, field: &str, value: &str) -> String {
    let lines: Vec<&str> = control.lines().collect();
    let end = lines
        .iter()
        .position(|line| line.trim().is_empty())
        .unwrap_or(lines.len());
    let prefix = format!("{}:", field.to_ascii_lowercase());
    let mut kept: Vec<String> = Vec::new();
    let mut skipping = false;
    for line in &lines[..end] {
        let continuation = line.starts_with([' ', '\t']);
        if continuation && skipping {
            continue;
        }
        skipping = !continuation && line.to_ascii_lowercase().starts_with(&prefix);
        if !skipping {
            kept.push((*line).to_string());
        }
    }
    kept.push(format!("{field}: {value}"));
    kept.extend(lines[end..].iter().map(|line| (*line).to_string()));
    let mut out = kept.join("\n");
    out.push('\n');
    out
}

/// `build-spec/pre-upstream.in`, kept by the importer when the hook creates
/// `debian/` in the orig. The tree cannot carry it: its `debian/` is the
/// packaged one.
fn vendored_pre_upstream(recipe: &Path) -> Option<PathBuf> {
    let hook = recipe.join(VENDORED_BUILD_SPEC).join("pre-upstream.in");
    hook.is_file().then_some(hook)
}

/// `build-spec/pre-build`, kept by the importer when the hook works on the
/// orig tarballs next to the source directory (it makes the `signed`, `noext`
/// and `component` origs and their signatures). The tree already shows what
/// the hook did to the sources, so only these hooks are vendored.
fn vendored_pre_build_for_orig(recipe: &Path) -> Option<PathBuf> {
    let hook = recipe.join(VENDORED_BUILD_SPEC).join("pre-build");
    hook.is_file().then_some(hook)
}

/// `build-spec/post-build`, the filled hook lintian's Makefile runs on the
/// built product (`$(post_build) $(product)`) after `dpkg-source` or
/// `dpkg-buildpackage`. The importer keeps it for every recipe that has one;
/// the product name is its first argument.
fn vendored_post_build(recipe: &Path) -> Option<PathBuf> {
    let hook = recipe.join(VENDORED_BUILD_SPEC).join("post-build");
    hook.is_file().then_some(hook)
}

/// Runs the vendored `post-build` hook on `product` as lintian's Makefile does:
/// in the build directory (the product's), with the product's name as the
/// argument. `LINTIAN_SOURCE_PATH` is where the files the hook reads from the
/// recipe (a signing key) are kept.
fn run_post_build(hook: &Path, product: &Path) {
    let dir = product
        .parent()
        .expect("product lives in the build directory");
    let name = product.file_name().expect("product has a name");
    let output = Command::new("sh")
        .arg(hook)
        .arg(name)
        .current_dir(dir)
        .env(
            "LINTIAN_SOURCE_PATH",
            hook.parent().expect("hook lives in build-spec"),
        )
        .output()
        .expect("spawning post-build hook");
    assert!(
        output.status.success(),
        "{}: post-build hook failed\nstdout:\n{}\nstderr:\n{}",
        hook.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs the vendored `pre-build` hook on the package directory, after the
/// main orig is packed and before `dpkg-source -b`, as lintian's upload
/// builder does. The hook reaches the orig tarballs through `$1/../`.
fn run_pre_build_for_orig(hook: &Path, source: &Path) {
    let output = Command::new("sh")
        .arg(hook)
        .arg(source)
        .output()
        .expect("spawning pre-build hook");
    assert!(
        output.status.success(),
        "{}: pre-build hook failed\nstdout:\n{}\nstderr:\n{}",
        hook.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The orig lintian's upload builder tars: the tree's upstream paths (no
/// `debian/`, no `added` root paths) with the `pre-upstream` hook run on top,
/// under `<source>-<upstream>/` like the real tarball. The hook is a lintian
/// template; its only variable here is `[% $source_path %]`, the tree (it holds
/// the packaged `debian/` the hook copies).
fn pack_orig_with_hook(tree: &Path, hook: &Path, orig: &Path, added: &[String]) {
    let work = orig.parent().expect("orig lives in the work directory");
    let name = orig
        .file_name()
        .and_then(OsStr::to_str)
        .expect("orig name is UTF-8");
    let top = name
        .strip_suffix(".orig.tar.gz")
        .expect("main orig name")
        .replacen('_', "-", 1);
    let stage = work.join("orig-stage");
    let base = work.join("orig-base.tar.gz");
    let package_dir = stage.join(&top);
    fs::create_dir_all(&package_dir)
        .unwrap_or_else(|error| panic!("creating {}: {error}", package_dir.display()));
    pack_orig_without_added(tree, &base, added);
    run_tar(&["-xzf"], &base, &package_dir, None);

    let template = fs::read_to_string(hook)
        .unwrap_or_else(|error| panic!("reading {}: {error}", hook.display()));
    let script = work.join("pre-upstream");
    fs::write(
        &script,
        template.replace("[% $source_path %]", &tree.display().to_string()),
    )
    .unwrap_or_else(|error| panic!("writing {}: {error}", script.display()));
    let output = Command::new("sh")
        .arg(&script)
        .arg(&package_dir)
        .output()
        .expect("spawning pre-upstream hook");
    assert!(
        output.status.success(),
        "{}: pre-upstream hook failed\nstdout:\n{}\nstderr:\n{}",
        hook.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    run_tar(&["-czf"], orig, &stage, Some(&top));
    fs::remove_dir_all(&stage)
        .unwrap_or_else(|error| panic!("removing {}: {error}", stage.display()));
    fs::remove_file(&base).unwrap_or_else(|error| panic!("removing {}: {error}", base.display()));
}

/// `tar` in `dir`: `-xzf` extracts the archive into it, `-czf` packs `member`
/// of it into the archive.
fn run_tar(mode: &[&str], archive: &Path, dir: &Path, member: Option<&str>) {
    let mut command = Command::new("tar");
    command.args(mode).arg(archive).arg("-C").arg(dir);
    command.args(member);
    let output = command.output().expect("spawning tar");
    assert!(
        output.status.success(),
        "tar {mode:?} {} failed\nstderr:\n{}",
        archive.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assemble_source_package(recipe: &Path, work: &Path) -> PathBuf {
    let source = work.join("src");
    copy_package_tree(recipe, &source);
    let pointers = debian_adds_pointers(recipe);
    let root_adds: Vec<String> = pointers
        .iter()
        .filter(|pointer| outside_debian(pointer))
        .cloned()
        .collect();
    let keep_ignored = !pointers.is_empty() || expects_tag_prefix(recipe, "source-contains-");
    let overrides = dpkg_overrides(recipe);
    let hook = vendored_pre_upstream(recipe);
    let pre_build = vendored_pre_build_for_orig(recipe);
    let non_native = recipe_is_non_native(recipe);

    // Everything the packing reads besides the tree: the `dpkg-source` flags,
    // the paths left out of the orig, whether the recipe is non-native (its
    // orig is always packed), and the `pre-upstream` and `pre-build` hooks. The
    // recipe's `eval/hints` reach the build only through `root_adds` and
    // `keep_ignored`.
    let mut key = BuildKey::new("dpkg-source");
    key.tree(&source);
    key.field("keep-ignored", &[u8::from(keep_ignored)]);
    key.field("overrides", overrides.join("\0").as_bytes());
    key.field("root-adds", root_adds.join("\0").as_bytes());
    let hook_text = hook.as_ref().map(|hook| {
        fs::read(hook).unwrap_or_else(|error| panic!("reading {}: {error}", hook.display()))
    });
    key.field("pre-upstream", hook_text.as_deref().unwrap_or_default());
    key.field("has-pre-upstream", &[u8::from(hook.is_some())]);
    key.field("non-native", &[u8::from(non_native)]);
    let pre_build_text = pre_build.as_ref().map(|hook| {
        fs::read(hook).unwrap_or_else(|error| panic!("reading {}: {error}", hook.display()))
    });
    key.field("pre-build", pre_build_text.as_deref().unwrap_or_default());
    key.field("has-pre-build", &[u8::from(pre_build.is_some())]);
    // The `post-build` hook of a `source-*` skeleton rewrites the `.dsc`; an
    // `upload-*` one rewrites the `.changes`, which this build does not make.
    let post_build = vendored_post_build(recipe).filter(|_| {
        recipe_skeleton(recipe).is_some_and(|skeleton| skeleton.starts_with("source-"))
    });
    let post_build_text = post_build.as_ref().map(|hook| {
        fs::read(hook).unwrap_or_else(|error| panic!("reading {}: {error}", hook.display()))
    });
    key.field("post-build", post_build_text.as_deref().unwrap_or_default());
    key.field("has-post-build", &[u8::from(post_build.is_some())]);

    let products = build_once(key.finish(), work, || {
        let packing = SourcePacking {
            keep_ignored,
            overrides: &overrides,
            root_adds: &root_adds,
            hook: hook.as_deref(),
            pre_build: pre_build.as_deref(),
            post_build: post_build.as_deref(),
            non_native,
        };
        pack_source_package(recipe, work, &source, packing);
        // The `.dsc` and the tarballs it lists, next to it; not the hook script
        // (it names the temporary directory it ran in).
        let mut files: Vec<PathBuf> = fs::read_dir(work)
            .unwrap_or_else(|error| panic!("reading {}: {error}", work.display()))
            .map(|entry| entry.expect("directory entry").path())
            .filter(|path| path.is_file() && path.file_name() != Some(OsStr::new("pre-upstream")))
            .collect();
        files.sort();
        files
    });
    products
        .into_iter()
        .find(|path| path.extension().and_then(OsStr::to_str) == Some("dsc"))
        .unwrap_or_else(|| panic!("{recipe:?}: no .dsc among the products"))
}

/// What packing a source package reads besides the tree (and so what its build
/// cache key covers).
#[derive(Clone, Copy)]
struct SourcePacking<'a> {
    keep_ignored: bool,
    overrides: &'a [String],
    root_adds: &'a [String],
    hook: Option<&'a Path>,
    pre_build: Option<&'a Path>,
    post_build: Option<&'a Path>,
    non_native: bool,
}

/// Pack `source` (a copy of the recipe tree in `work`) with `dpkg-source`,
/// synthesizing the orig tarball when the format asks for one.
fn pack_source_package(recipe: &Path, work: &Path, source: &Path, packing: SourcePacking<'_>) {
    let SourcePacking {
        keep_ignored,
        overrides,
        root_adds,
        hook,
        pre_build,
        post_build,
        non_native,
    } = packing;
    if let Some(hook) = hook {
        let name = main_orig_name(source);
        pack_orig_with_hook(source, hook, &work.join(&name), root_adds);
    } else if non_native || !root_adds.is_empty() {
        // lintian's upload builder always tars the orig of a non-native
        // recipe, from `orig/` alone, which is empty when the recipe has no
        // upstream files. Without it `dpkg-source` builds a 1.0 package
        // as a native one, with no `.diff.gz`.
        let name = main_orig_name(source);
        pack_orig_without_added(source, &work.join(&name), root_adds);
    }
    if let Some(pre_build) = pre_build {
        run_pre_build_for_orig(pre_build, source);
    }
    let (mut ok, mut stdout, mut stderr) = dpkg_source_build(work, keep_ignored, overrides);
    let mut orig_name = None;
    if !ok && let Some(name) = missing_upstream_tarball(&stderr) {
        assert!(
            !is_component_orig(&name),
            "{recipe:?}: component orig {name} is not synthesized"
        );
        pack_main_orig(source, &work.join(&name));
        (ok, stdout, stderr) = dpkg_source_build(work, keep_ignored, overrides);
        orig_name = Some(name);
    } else if hook.is_none() && (non_native || !root_adds.is_empty()) {
        orig_name = Some(main_orig_name(source));
    }
    if !ok
        && stderr.contains("unexpected upstream changes")
        && let Some(name) = &orig_name
    {
        // dpkg-source mis-reads an orig whose last member name contains a
        // quote (`data/".tif` in legacy-filenames) and reports those files as
        // new. A later member lets it see the archive correctly. The pad is in
        // the tree and the orig, so it is not a diff.
        fs::write(source.join("zz-pad"), "\n").expect("writing orig pad");
        pack_orig_without_added(source, &work.join(name), root_adds);
        (ok, stdout, stderr) = dpkg_source_build(work, keep_ignored, overrides);
    }
    assert!(ok, "{recipe:?}: dpkg-source -b failed\n{stdout}\n{stderr}");
    let dsc = find_dsc(work);
    if let Some(post_build) = post_build {
        run_post_build(post_build, &dsc);
    }
}

pub fn assert_source_package_parity(recipe_rel: &str) {
    if let Some(reason) = skipped_recipe_reason(recipe_rel) {
        eprintln!("{recipe_rel}: skipped, {reason}");
        return;
    }
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
    rewrite_changelog_last_used(recipe_rel, &mut actual);
    rewrite_current_policy_release_date(recipe_rel, &mut actual);
    rewrite_current_policy_version(recipe_rel, &mut actual);
    drop_hints_for_unvendorable_files(recipe_rel, &mut actual);
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (source package)"
    );
}

/// `binaries-multiarch-wrong-dir`'s lintian `eval/post-test` rewrites
/// `is for i386 instead of amd64` (either way round) to
/// `INCOMPATIBILITY STATEMENT`. The vendored hint keeps that token; the hook
/// is not copied into the tree.
fn rewrite_incompatibility_statement(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/files/architecture/binaries-multiarch-wrong-dir" {
        return;
    }
    for hint in hints {
        if hint.tag == "triplet-dir-and-architecture-mismatch" && hint.extra.starts_with("is for ")
        {
            hint.extra = "INCOMPATIBILITY STATEMENT".to_string();
        }
    }
}

/// `filename-length-really-really-really-really-long-package-name`'s lintian
/// `eval/post-test` rewrites the architecture at the end of the `.changes` and
/// `.buildinfo` names to `ARCHITECTURE`; the host builds for its own
/// architecture, so the same rewrite is applied to the lint output.
fn rewrite_architecture_in_group_names(recipe_rel: &str, hints: &mut [(String, ParityHint)]) {
    if recipe_rel
        != "checks/archive/file/name/length/filename-length-really-really-really-really-long-package-name"
    {
        return;
    }
    for (processable_type, hint) in hints {
        if !matches!(processable_type.as_str(), "changes" | "buildinfo") {
            continue;
        }
        let suffix = format!(".{processable_type}");
        let Some(stem) = hint.extra.strip_suffix(&suffix) else {
            continue;
        };
        if let Some((head, arch)) = stem.rsplit_once('_')
            && arch.chars().all(|c| c.is_ascii_alphanumeric())
        {
            hint.extra = format!("{head}_ARCHITECTURE{suffix}");
        }
    }
}

/// `control-files-bad`'s `tar-control` hook packs the control members with
/// `--owner=nobody` and no `--group`, so the group is the primary group of
/// whoever runs the test (lintian's own runs are root). The vendored hint
/// expects `nobody/root`; map the host's group to `root`.
fn rewrite_control_file_group(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/control-files/control-files-bad" {
        return;
    }
    for hint in hints {
        if hint.tag != "control-file-has-bad-owner" {
            continue;
        }
        let Some(rest) = hint.extra.strip_prefix("nobody/") else {
            continue;
        };
        let Some((_, tail)) = rest.split_once(" != ") else {
            continue;
        };
        hint.extra = format!("nobody/root != {tail}");
    }
}

/// `fields-perl-provides`'s lintian `eval/post-test` rewrites the version in
/// `with <version>` to `VERSION`, because the Perl core version differs between
/// archives. The vendored hint keeps that token; the hook is not copied.
fn rewrite_perl_core_version(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/languages/perl/core/provides/fields-perl-provides" {
        return;
    }
    for hint in hints {
        if hint.tag == "package-superseded-by-perl" && hint.extra.starts_with("with ") {
            hint.extra = "with VERSION".to_string();
        }
    }
}

/// `files-fonts`'s lintian `eval/post-test` rewrites the packages listed in
/// `also in (…)` to `FONT_PACKAGES`, because the archive's font list differs
/// between runs. The vendored hint keeps that token; the hook is not copied.
fn rewrite_font_package_list(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/fonts/files-fonts" {
        return;
    }
    for hint in hints {
        if hint.tag != "duplicate-font-file" {
            continue;
        }
        let Some(start) = hint.extra.find("also in (") else {
            continue;
        };
        let Some(relative_end) = hint.extra[start..].find(')') else {
            continue;
        };
        let end = start + relative_end;
        hint.extra
            .replace_range(start..=end, "also in (FONT_PACKAGES)");
    }
}

/// `testsuite-general`'s lintian `pre-build` makes the named pipe
/// `debian/tests/fifo`; `lfifo` links to it. A pipe cannot be vendored, so the
/// filled tree lacks both and the Rule reports them missing. Lintian reports
/// them as not regular files, a Tag we do not have.
fn drop_hints_for_unvendorable_files(recipe_rel: &str, hints: &mut Vec<ParityHint>) {
    if recipe_rel != "checks/testsuite/testsuite-general" {
        return;
    }
    hints.retain(|hint| {
        !(hint.tag == "missing-runtime-test-file"
            && matches!(
                hint.extra.as_str(),
                "debian/tests/fifo" | "debian/tests/lfifo"
            ))
    });
}

/// The Tags lintian 2.144's `files/source-missing` Check emits only as
/// `pointed_hint(tag, $item->pointer)`: an empty extra and the path as the
/// pointer, printed `tag [path]`. Their recipes under `checks/files/source-missing/`
/// carry that form. `checks/cruft/cruft-general-upstream` and
/// `checks/cruft/cruft-python` carry the bare `tag path`, expectations from an
/// older lintian that 2.144.0's code can no longer produce. The pointer form is
/// the truth, so a bare path extra of exactly these three Tags is compared as
/// an empty extra with the path as Location (`[path]`, as [`strip_location_pointer`]
/// leaves a lone pointer). Other Tags, which may carry bare paths for real, are
/// not touched; the vendored hint files stay as lintian shipped them.
const POINTER_ONLY_SOURCE_MISSING_TAGS: [&str; 3] = [
    "source-is-missing",
    "source-contains-prebuilt-javascript-object",
    "source-contains-prebuilt-python-object",
];

fn point_bare_source_missing_hints(hints: &mut [ParityHint]) {
    for hint in hints {
        if POINTER_ONLY_SOURCE_MISSING_TAGS.contains(&hint.tag.as_str())
            && !hint.extra.is_empty()
            && !hint.extra.starts_with('[')
        {
            hint.extra = format!("[{}]", hint.extra);
        }
    }
}

/// Two vendored recipes carry `eval/hints` lines from before lintian printed
/// pointers and the full extra, which lintian 2.144's Perl no longer emits.
/// The expected hints are brought to what that Perl prints; the Rules follow
/// the Perl, as the sibling recipes (`cruft-general-test-suite`,
/// `version-substvars-general`) confirm.
///
/// - `cruft-general-upstream` (a lintian `Todo` test): the bare
///   `configure-generated-file-in-source config.cache` is `[config.cache]`
///   (`pointed_hint` with a pointer and no note), and `source-is-missing` is
///   also reported for the `.opt-1.pyc`.
/// - `version-substvars-obsolete`: the Perl's `version-substvar-for-external-package`
///   extra is `<field> <substvar> <installable> -> <pkg>`, and its regex only
///   accepts `${source:…}` / `${binary:…}`, so `foreign-pkg:any (= ${Source-Version})`
///   yields no hint. Only `Replaces ${binary:Version} program-data ->
///   other-foreign-pkg` remains. `substvar-source-version-is-deprecated` is
///   `program-data Depends` (the Perl prints installable and field).
fn correct_stale_expected_hints(recipe: &Path, hints: &mut Vec<ParityHint>) {
    point_bare_source_missing_hints(hints);
    if recipe.ends_with("checks/cruft/cruft-general-upstream") {
        for hint in hints.iter_mut() {
            if hint.tag == "configure-generated-file-in-source" && hint.extra == "config.cache" {
                hint.extra = "[config.cache]".to_string();
            }
        }
        // The Check looks for `__pycache__/fake.cpython-38.opt-1.py` as the
        // source of `fake.cpython-38.opt-1.pyc` (its pattern cuts only
        // `.cpython-38` before `.pyc`, not `.opt-1`) and finds none, so it
        // reports the missing source of that file as well; the vendored hints
        // list only the plain `.pyc`.
        let opt1 = ParityHint {
            tag: "source-contains-prebuilt-python-object".to_string(),
            extra: "[__pycache__/fake.cpython-38.opt-1.pyc]".to_string(),
        };
        if hints.contains(&opt1) {
            hints.push(ParityHint {
                tag: "source-is-missing".to_string(),
                ..opt1
            });
        }
    } else if recipe.ends_with("checks/debian/version-substvars/version-substvars-obsolete") {
        let tag = "version-substvar-for-external-package";
        if hints.iter().any(|hint| hint.tag == tag) {
            hints.retain(|hint| hint.tag != tag);
            hints.push(ParityHint {
                tag: tag.to_string(),
                extra: "Replaces ${binary:Version} program-data -> other-foreign-pkg".to_string(),
            });
        }
        // The Perl's `substvar-source-version-is-deprecated` is `$installable, $field`.
        for hint in hints.iter_mut() {
            if hint.tag == "substvar-source-version-is-deprecated" && hint.extra == "program-data" {
                hint.extra = "program-data Depends".to_string();
            }
        }
    }
}

/// `standards-version-timewarp`'s lintian `eval/post-test` rewrites the date of
/// the current Policy release, the newest in the vendored table, to `CURRENT`.
/// The vendored hint keeps that token; the hook is not copied into the tree.
fn rewrite_current_policy_release_date(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/fields/standards-version/standards-version-timewarp" {
        return;
    }
    for hint in hints {
        if hint.tag != "timewarp-standards-version" {
            continue;
        }
        let Some(start) = hint.extra.find(" < ") else {
            continue;
        };
        let Some(relative_end) = hint.extra[start..].find(')') else {
            continue;
        };
        let end = start + relative_end;
        hint.extra.replace_range(start..end, " < CURRENT");
    }
}

/// The Standards-Version recipes (and the tracking recipe that carries the same
/// hint) have lintian `eval/post-test` rewrite the newest
/// Policy release in `(current is …)` to `CURRENT`, for the newer and the
/// out-of-date Tags. The vendored hints keep that token; the hook is not
/// copied into the tree.
fn rewrite_current_policy_version(_recipe_rel: &str, hints: &mut [ParityHint]) {
    for hint in hints {
        if hint.tag != "newer-standards-version" && hint.tag != "out-of-date-standards-version" {
            continue;
        }
        let Some(start) = hint.extra.find("(current is ") else {
            continue;
        };
        let Some(relative_end) = hint.extra[start..].find(')') else {
            continue;
        };
        let end = start + relative_end;
        hint.extra
            .replace_range(start..=end, "(current is CURRENT)");
    }
}

/// `changelog-file-general`'s lintian `eval/post-test` rewrites the date in
/// `(last used: …)` to `CURRENT`. The vendored hints keep that token; the
/// hook itself is not copied into the recipe tree.
fn rewrite_changelog_last_used(recipe_rel: &str, hints: &mut [ParityHint]) {
    if recipe_rel != "checks/debian/changelog/changelog-file-general" {
        return;
    }
    for hint in hints {
        let Some(start) = hint.extra.find("(last used: ") else {
            continue;
        };
        let Some(relative_end) = hint.extra[start..].find(')') else {
            continue;
        };
        let end = start + relative_end;
        hint.extra
            .replace_range(start..=end, "(last used: CURRENT)");
    }
}

// --- Group Parity (ADR 0055) -------------------------------------------------------------------

/// Tags of the Rules that inspect the artifacts of a Group, by Rule kind.
pub struct GroupTags {
    changes: HashSet<String>,
    buildinfo: HashSet<String>,
    group: HashSet<String>,
}

pub fn catalogued_group_tags() -> GroupTags {
    GroupTags {
        changes: catalogued_tags_with_impl(&["impl ChangesRule for"]),
        buildinfo: catalogued_tags_with_impl(&["impl BuildinfoRule for"]),
        group: catalogued_tags_with_impl(&["impl GroupRule for"]),
    }
}

/// A hint a Group lint shows, per processable type: `changes` and `buildinfo`
/// hints come from the Changes-file and Buildinfo-file Rules; a Group Rule's
/// Diagnostics attach to the Source package, so its hints are `source`. Kept
/// in step with `applies_to_group` in `scripts/import_lintian_parity.py`.
fn applies_to_group(processable_type: &str, tag: &str, tags: &GroupTags) -> bool {
    match processable_type {
        "changes" => tags.changes.contains(tag),
        "buildinfo" => tags.buildinfo.contains(tag),
        "source" => tags.group.contains(tag),
        _ => false,
    }
}

/// The skeleton a recipe was filled from (`ORIGIN`).
fn recipe_skeleton(recipe: &Path) -> Option<String> {
    let origin = fs::read_to_string(recipe.join("ORIGIN")).ok()?;
    origin
        .lines()
        .find_map(|line| line.strip_prefix("Skeleton:"))
        .map(|skeleton| skeleton.trim().to_string())
}

/// Whether lintian's skeleton for the recipe builds a non-native package
/// (`Type: non-native`): its upload and source builders tar an orig first.
/// The `upload-native` and `source-native` skeletons are the native ones.
fn recipe_is_non_native(recipe: &Path) -> bool {
    matches!(
        recipe_skeleton(recipe).as_deref(),
        Some("upload-non-native" | "upload-builder-only" | "source-non-native")
    )
}

/// Recipes whose build product is a `.changes`, which lintian lints as a Group:
/// the `upload-*` skeletons build one with `dpkg-buildpackage`, the `changes`
/// skeleton ships a filled one.
fn recipe_product_is_changes(recipe: &Path) -> bool {
    matches!(
        recipe_skeleton(recipe).as_deref(),
        Some("upload-native" | "upload-non-native" | "upload-builder-only" | "changes")
    )
}

fn catalogued_group_hints(recipe: &Path, tags: &GroupTags) -> Vec<(String, ParityHint)> {
    let mut expected: Vec<(String, ParityHint)> = load_hints(&recipe.join("eval/hints"))
        .into_iter()
        .filter(|(processable_type, hint)| applies_to_group(processable_type, &hint.tag, tags))
        .collect();
    expected.sort();
    expected
}

/// Vendored recipes whose product is a `.changes` and whose hints mention a
/// catalogued Tag with Changes-file, Buildinfo-file or Group Applicability.
pub fn catalogued_group_recipe_rels() -> Vec<String> {
    let root = parity_suite_root();
    let tags = catalogued_group_tags();
    let mut rels = Vec::new();
    for recipe in discover_parity_recipes(&root) {
        if !recipe_product_is_changes(&recipe) {
            continue;
        }
        if catalogued_group_hints(&recipe, &tags).is_empty() {
            continue;
        }
        rels.push(recipe_rel(&root, &recipe));
    }
    rels
}

fn find_changes(dir: &Path) -> PathBuf {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            path.is_file() && path.extension().and_then(OsStr::to_str) == Some("changes")
        })
        .collect();
    found.sort();
    assert_eq!(
        found.len(),
        1,
        "expected one .changes in {}, found {found:?}",
        dir.display()
    );
    found.remove(0)
}

/// `dpkg-buildpackage` as lintian's upload builder runs it: source and binary
/// packages, unsigned, VCS cruft kept. Returns the success flag and the output
/// of both streams, where `dpkg-source` reports a missing orig tarball.
fn dpkg_build_upload(source: &Path) -> (bool, String) {
    ensure_rules_executable(source);
    let mut command = dpkg_buildpackage_command(source, "nocheck");
    command
        .args(["-us", "-uc", "-d"])
        .args(["-iNEVER_MATCH_ANYTHING", "-INEVER_MATCH_ANYTHING"])
        .arg("--source-option=--auto-commit");
    let output = command.output().expect("spawning dpkg-buildpackage");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

/// Build the product of a Group recipe in `work` and return its `.changes`:
/// a filled `changes` skeleton is put together by its own Makefile (it copies
/// the referenced files next to `test.changes`); an `upload-*` recipe is built
/// with `dpkg-buildpackage`, packing a main orig tarball when it asks for one
/// (as the Source-package harness does), and the recipe's `post-build` hook
/// then rewrites the `.changes`.
fn assemble_group(recipe: &Path, work: &Path) -> PathBuf {
    if recipe_skeleton(recipe).as_deref() == Some("changes") {
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
        return find_changes(&build);
    }

    let source = work.join("src");
    copy_package_tree(recipe, &source);
    let (ok, text) = dpkg_build_upload(&source);
    if !ok {
        let Some(name) = missing_upstream_tarball(&text) else {
            panic!("{recipe:?}: dpkg-buildpackage failed\n{text}");
        };
        assert!(
            !is_component_orig(&name),
            "{recipe:?}: component orig {name} is not synthesized"
        );
        pack_main_orig(&source, &work.join(&name));
        let (ok, text) = dpkg_build_upload(&source);
        assert!(
            ok,
            "{recipe:?}: dpkg-buildpackage failed after orig {name}\n{text}"
        );
    }
    let changes = find_changes(work);
    if let Some(post_build) = vendored_post_build(recipe) {
        run_post_build(&post_build, &changes);
    }
    changes
}

/// Build the recipe's Group, lint its `.changes` as lintian's runner does, and
/// compare the hints per processable type.
pub fn assert_group_parity(recipe_rel: &str) {
    if let Some(reason) = skipped_recipe_reason(recipe_rel) {
        eprintln!("{recipe_rel}: skipped, {reason}");
        return;
    }
    let recipe = parity_suite_root().join(recipe_rel);
    if let Some(architectures) = unmet_test_architectures(&recipe) {
        eprintln!("{recipe_rel}: skipped, the recipe is only built for {architectures}");
        return;
    }
    let tags = catalogued_group_tags();
    let expected = catalogued_group_hints(&recipe, &tags);
    assert!(
        !expected.is_empty(),
        "{recipe_rel}: listed Parity recipe has no catalogued Group hints"
    );

    let work = tempfile::tempdir().expect("temp recipe workdir");
    let changes = assemble_group(&recipe, work.path());

    let mut selected: Vec<String> = expected
        .iter()
        .map(|(_, hint)| hint.tag.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    selected.sort();
    let mut extra_args = Vec::new();
    for tag in &selected {
        extra_args.push("--select");
        extra_args.push(tag.as_str());
    }

    let (_status, stdout, stderr) = run_lint_subject(&changes, &extra_args);
    assert!(
        stderr.is_empty() || !stdout.is_empty(),
        "{recipe_rel}: lint failed with empty stdout\n{stderr}"
    );

    let mut actual: Vec<(String, ParityHint)> = stdout
        .lines()
        .filter_map(parse_diagnostic_line)
        .filter(|(processable_type, hint)| applies_to_group(processable_type, &hint.tag, &tags))
        .collect();
    rewrite_architecture_in_group_names(recipe_rel, &mut actual);
    actual.sort();

    assert_eq!(actual, expected, "Parity mismatch for {recipe_rel} (group)");
}
