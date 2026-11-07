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

/// `debhelper-compat-missing` keeps `debian/compat` so the package builds, and
/// `debian/source/options` tar-ignores it. The `(source)` hints
/// `package-uses-deprecated-debhelper-compat-version` and
/// `debhelper-base-compat-level-is-missing` are from the packed source, where
/// the file is gone. The filled tree still has compat 13, so Source-tree
/// Parity must not expect them. Source-package Parity packs with `dpkg-source`
/// and still expects them. Kept in step with
/// `source_tree_hint_matches_filled_tree` in `scripts/import_lintian_parity.py`.
fn source_tree_hint_matches_filled_tree(recipe: &Path, hint: &ParityHint) -> bool {
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
            catalogued.contains(&hint.tag)
                && applies_to_source_tree(processable_type, &hint.extra)
                && source_tree_hint_matches_filled_tree(recipe, hint)
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

    // The recipe directory also holds ORIGIN and eval/, which are not part of
    // the Source tree lintian saw; lint a copy without them.
    let work = tempfile::tempdir().expect("temp recipe workdir");
    let source = work.path().join("src");
    copy_package_tree(&recipe, &source);

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
    let output = Command::new("perl")
        .args(["-MConfig", "-wE", "say substr($Config{vendorarch},1)"])
        .output()
        .expect("run perl to find the vendor architecture directory");
    let vendorarch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    extra.replace(PLACEHOLDER, &vendorarch)
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
    fs::write(
        &wrapper,
        r#"#!/usr/bin/python3
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
"#,
    )
    .expect("writing dpkg-deb wrapper");
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
    fs::read_to_string(source.join("debian/rules"))
        .is_ok_and(|text| text.contains("--with nodejs") || text.contains("dh-sequence-nodejs"))
}

fn dpkg_build_binary(source: &Path) {
    ensure_rules_executable(source);
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
        .args(["-b", "-us", "-uc", "-d"])
        .env("DEB_BUILD_OPTIONS", "nocheck noautodbgsym")
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

fn dpkg_source_build(work: &Path, keep_ignored: bool) -> (bool, String, String) {
    let mut command = Command::new("dpkg-source");
    command.current_dir(work).arg("-b");
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
    let Ok(text) = fs::read_to_string(recipe.join("eval/hints")) else {
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

fn assemble_source_package(recipe: &Path, work: &Path) -> PathBuf {
    let source = work.join("src");
    copy_package_tree(recipe, &source);
    let pointers = debian_adds_pointers(recipe);
    let root_adds: Vec<String> = pointers
        .iter()
        .filter(|pointer| outside_debian(pointer))
        .cloned()
        .collect();
    let keep_ignored = !pointers.is_empty();
    if !root_adds.is_empty() {
        let name = main_orig_name(&source);
        pack_orig_without_added(&source, &work.join(&name), &root_adds);
    }
    let (ok, stdout, stderr) = dpkg_source_build(work, keep_ignored);
    if !ok {
        let Some(name) = missing_upstream_tarball(&stderr) else {
            panic!("{recipe:?}: dpkg-source -b failed\n{stdout}\n{stderr}");
        };
        assert!(
            !is_component_orig(&name),
            "{recipe:?}: component orig {name} is not synthesized"
        );
        pack_main_orig(&source, &work.join(&name));
        let (ok, stdout, stderr) = dpkg_source_build(work, keep_ignored);
        if !ok && stderr.contains("unexpected upstream changes") {
            // dpkg-source mis-reads an orig whose last member name contains a
            // quote (`data/".tif` in legacy-filenames) and reports those
            // files as new. A later member lets it see the archive correctly.
            // The pad is in the tree and the orig, so it is not a diff.
            fs::write(source.join("zz-pad"), "\n").expect("writing orig pad");
            pack_main_orig(&source, &work.join(&name));
            let (ok, stdout, stderr) = dpkg_source_build(work, keep_ignored);
            assert!(
                ok,
                "{recipe:?}: dpkg-source -b failed after orig {name}\n{stdout}\n{stderr}"
            );
        } else {
            assert!(
                ok,
                "{recipe:?}: dpkg-source -b failed after orig {name}\n{stdout}\n{stderr}"
            );
        }
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
    rewrite_changelog_last_used(recipe_rel, &mut actual);
    actual.sort();

    assert_eq!(
        actual, expected,
        "Parity mismatch for {recipe_rel} (source package)"
    );
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
