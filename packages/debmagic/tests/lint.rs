//! Integration tests for `debmagic lint` via the built binary.

mod common;

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use common::{
    ParityHint, assets_dir, build_native_dsc, missing_upstream_tarball, parse_diagnostic_line,
    parse_overridden_line, parse_universal_hint_line, run_lint, run_lint_subject,
    run_lint_subjects, source_rule_counts, temp_artifact,
};

#[test]
fn lint_dummy_hit_prints_diagnostic_and_succeeds() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(stdout.contains("DM0001"), "stdout: {stdout}");
    assert!(
        stdout.contains("debmagic-dummy-trigger"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("debian/debmagic-dummy-lint"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Found 1 warning."),
        "expected ruff-style summary, stdout: {stdout}"
    );
    assert!(!stdout.contains("All checks passed!"), "stdout: {stdout}");
    assert_finished(&stdout, source_rule_counts().default);
}

#[test]
fn lint_override_flags_are_accepted_and_change_nothing_yet() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let without_finished = |stdout: &str| -> Vec<String> {
        stdout
            .lines()
            .filter(|line| !line.starts_with("Finished in "))
            .map(str::to_string)
            .collect()
    };
    let (status, plain, stderr) = run_lint(&source_dir, &[]);
    assert!(status.success(), "stderr: {stderr}");
    for flag in ["--show-overrides", "--no-override"] {
        let (status, stdout, stderr) = run_lint(&source_dir, &[flag]);
        assert!(status.success(), "{flag}: stderr: {stderr}");
        assert_eq!(
            without_finished(&stdout),
            without_finished(&plain),
            "{flag}"
        );
    }
}

#[test]
fn lint_override_flags_conflict() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &["--show-overrides", "--no-override"]);
    assert_eq!(status.code(), Some(2), "stdout: {stdout}");
    assert!(stdout.is_empty(), "stdout: {stdout}");
    assert!(stderr.contains("cannot be used with"), "stderr: {stderr}");
}

#[test]
fn lint_timings_prints_initialization_and_each_rule() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &["--timings", "--select", "DM0001"]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    let init_at = stdout
        .find("Initialization\n")
        .expect("initialization section");
    let rules_at = stdout.find("\nRules\n").expect("rules section");
    assert!(init_at < rules_at, "{stdout}");
    assert!(stdout.contains("\n  bringup"), "{stdout}");
    assert!(stdout.contains("\n  shared"), "{stdout}");
    assert!(
        stdout.contains("DM0001  debmagic-dummy-trigger"),
        "{stdout}"
    );
    assert_finished(&stdout, 1);
}

#[test]
fn lint_dummy_miss_succeeds_without_diagnostic() {
    let source_dir = assets_dir().join("check_dummy_miss");
    let (status, stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(
        !stdout.contains("DM0001"),
        "unexpected diagnostic on stdout: {stdout}"
    );
    assert!(
        stdout.contains("All checks passed!"),
        "expected success summary, stdout: {stdout}"
    );
    assert_finished(&stdout, source_rule_counts().default);
}

#[test]
fn lint_ignore_excludes_dummy_rule() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &["--ignore", "DM0001"]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(
        !stdout.contains("DM0001"),
        "ignored rule still printed: {stdout}"
    );
    assert!(
        stdout.contains("All checks passed!"),
        "expected success summary, stdout: {stdout}"
    );
    assert_finished(&stdout, source_rule_counts().default - 1);
}

#[test]
fn lint_select_prefix_excludes_dummy_rule() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &["--select", "LN"]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(
        !stdout.contains("DM0001"),
        "unselected rule still printed: {stdout}"
    );
    assert!(
        stdout.contains("All checks passed!"),
        "expected success summary, stdout: {stdout}"
    );
    assert_finished(&stdout, source_rule_counts().ln);
}

#[test]
fn lint_missing_debian_dir_fails_on_stderr() {
    let source_dir = assets_dir();
    let (status, _stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(
        !status.success(),
        "expected failure for directory without debian/"
    );
    assert!(!stderr.is_empty(), "expected error message on stderr");
}

#[test]
fn lint_fail_on_warning_exits_nonzero_but_prints_diagnostic() {
    let source_dir = assets_dir().join("check_dummy_hit");
    let (status, stdout, stderr) = run_lint(&source_dir, &["--fail-on", "warning"]);

    assert!(
        !status.success(),
        "expected policy failure, stderr: {stderr}"
    );
    assert_eq!(status.code(), Some(1));
    assert!(stdout.contains("DM0001"), "stdout: {stdout}");
    assert!(
        stdout.contains("debmagic-dummy-trigger"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Found 1 warning."),
        "expected ruff-style summary, stdout: {stdout}"
    );
    assert_finished(&stdout, source_rule_counts().default);
}

#[test]
fn lint_binary_package_via_positional_fails_on_malformed_archive() {
    let deb = temp_artifact("deb");
    let (status, _stdout, stderr) = run_lint_subject(&deb, &[]);
    let _ = fs::remove_file(&deb);

    assert!(
        !status.success(),
        "expected failure for malformed .deb Subject"
    );
    assert!(
        stderr.contains("control.tar") || stderr.contains("ar") || stderr.contains("archive"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_binary_package_via_source_dir_fails_on_malformed_archive() {
    let deb = temp_artifact("deb");
    let (status, _stdout, stderr) = run_lint(&deb, &[]);
    let _ = fs::remove_file(&deb);

    assert!(
        !status.success(),
        "expected failure for malformed .deb Subject"
    );
    assert!(
        stderr.contains("control.tar") || stderr.contains("ar") || stderr.contains("archive"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_source_package_via_positional_fails_on_empty_dsc() {
    let dsc = temp_artifact("dsc");
    let (status, _stdout, stderr) = run_lint_subject(&dsc, &[]);
    let _ = fs::remove_file(&dsc);

    assert!(!status.success(), "expected failure for empty .dsc Subject");
    assert!(
        stderr.contains("Deb822")
            || stderr.contains("paragraph")
            || stderr.contains("files")
            || stderr.contains("readable"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_source_package_via_source_dir_fails_on_empty_dsc() {
    let dsc = temp_artifact("dsc");
    let (status, _stdout, stderr) = run_lint(&dsc, &[]);
    let _ = fs::remove_file(&dsc);

    assert!(!status.success(), "expected failure for empty .dsc Subject");
    assert!(
        stderr.contains("Deb822")
            || stderr.contains("paragraph")
            || stderr.contains("files")
            || stderr.contains("readable"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_source_package_native_dsc_detects_windows_binary() {
    let work = tempfile::tempdir().expect("workdir");
    let exe = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/lintian-parity/checks/cruft/cruft-general-upstream/fake-win32-bin.exe"),
    )
    .expect("fake windows binary");
    let dsc = build_native_dsc(work.path(), "example", &[("bin/app.exe", exe.as_slice())]);
    let (status, stdout, stderr) = run_lint_subject(
        &dsc,
        &["--select", "source-contains-prebuilt-windows-binary"],
    );

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(
        stdout.contains("source-contains-prebuilt-windows-binary (LN0004)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("bin/app.exe"), "stdout: {stdout}");
    assert!(stdout.contains("example (source)"), "stdout: {stdout}");
}

#[test]
fn lint_source_package_select_required_field_applies() {
    let work = tempfile::tempdir().expect("workdir");
    let dsc = build_native_dsc(work.path(), "example", &[]);
    let (status, stdout, stderr) = run_lint_subject(&dsc, &["--select", "required-field"]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(
        !stdout.contains("required-field"),
        "complete debian/control should not emit required-field, stdout: {stdout}"
    );
}

/// A `.changes` for the native `example_1.0` Source package in `dir`, naming
/// the files in `files` as `(section, name)`. Every Policy 5.5 field is
/// declared, so `required-field` only reports what a test sets up.
fn write_changes(
    dir: &std::path::Path,
    name: &str,
    source: &str,
    files: &[(&str, &str)],
) -> PathBuf {
    let mut text = format!(
        "Format: 1.8\nDate: Mon, 1 Jan 2024 00:00:00 +0000\nSource: {source}\n\
         Architecture: source\nVersion: 1.0\nDistribution: unstable\n\
         Maintainer: Example <ex@example.com>\nChanges: example\n\
         Checksums-Sha1: x\nChecksums-Sha256: x\nFiles:\n"
    );
    for (section, file) in files {
        text.push_str(&format!(
            " d41d8cd98f00b204e9800998ecf8427e 0 {section} optional {file}\n"
        ));
    }
    let path = dir.join(name);
    fs::write(&path, text).expect("writing a changes file");
    path
}

#[test]
fn lint_changes_pulls_in_its_dsc_and_reports_the_bad_section() {
    let work = tempfile::tempdir().expect("workdir");
    let dsc = build_native_dsc(work.path(), "example", &[]);
    assert!(dsc.is_file());
    let changes = write_changes(
        work.path(),
        "example_1.0_source.changes",
        "example",
        &[
            ("non-free", "example_1.0.dsc"),
            ("utils", "example_1.0.tar.xz"),
        ],
    );
    let (status, stdout, stderr) = run_lint_subject(
        &changes,
        &["--select", "bad-section-in-changes-file,required-field"],
    );

    assert_eq!(status.code(), Some(1), "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stdout.contains(
            "E: example (changes): bad-section-in-changes-file (LN1440) example_1.0.dsc non-free"
        ),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Found 1 error."), "stdout: {stdout}");
    // The Changes Rule and the Source-package Rule both ran: the `.dsc` the
    // `.changes` names is part of the Group.
    assert_finished(&stdout, 2);
}

#[test]
fn lint_merges_arguments_of_one_source_package_into_a_group() {
    let work = tempfile::tempdir().expect("workdir");
    let dsc = build_native_dsc(work.path(), "example", &[]);
    let changes = write_changes(
        work.path(),
        "example_1.0_source.changes",
        "example",
        &[
            ("non-free", "example_1.0.dsc"),
            ("utils", "example_1.0.tar.xz"),
        ],
    );
    // The `.dsc` and the `.changes` that names it are one Group: one report of
    // each Rule, not one per argument.
    let (status, stdout, stderr) = run_lint_subjects(
        &[&changes, &dsc],
        &["--select", "bad-section-in-changes-file,required-field"],
    );
    assert_eq!(status.code(), Some(1), "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(
        stdout
            .matches("bad-section-in-changes-file (LN1440)")
            .count(),
        1,
        "stdout: {stdout}"
    );
    assert_finished(&stdout, 2);
}

#[test]
fn lint_changes_naming_a_missing_file_fails() {
    let work = tempfile::tempdir().expect("workdir");
    let changes = write_changes(
        work.path(),
        "example_1.0_source.changes",
        "example",
        &[("utils", "gone_1.0_all.deb")],
    );
    let (status, _stdout, stderr) = run_lint_subject(&changes, &[]);
    assert_eq!(status.code(), Some(1));
    assert!(
        stderr.contains("gone_1.0_all.deb does not exist"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_two_changes_files_make_two_groups() {
    let first = tempfile::tempdir().expect("workdir");
    let second = tempfile::tempdir().expect("workdir");
    let one = write_changes(
        first.path(),
        "one_1.0_source.changes",
        "one",
        &[("non-free", "a_1.0.tar.xz")],
    );
    let two = write_changes(
        second.path(),
        "two_1.0_source.changes",
        "two",
        &[("contrib", "b_1.0.tar.xz")],
    );
    fs::write(first.path().join("a_1.0.tar.xz"), b"").expect("tarball");
    fs::write(second.path().join("b_1.0.tar.xz"), b"").expect("tarball");
    let (status, stdout, stderr) =
        run_lint_subjects(&[&one, &two], &["--select", "bad-section-in-changes-file"]);
    assert_eq!(status.code(), Some(1), "stderr: {stderr}");
    assert!(
        stdout.contains(
            "E: one (changes): bad-section-in-changes-file (LN1440) a_1.0.tar.xz non-free"
        ),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(
            "E: two (changes): bad-section-in-changes-file (LN1440) b_1.0.tar.xz contrib"
        ),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Found 2 errors."), "stdout: {stdout}");
}

#[test]
fn lint_source_tree_cannot_be_mixed_with_artifacts() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_debmagic"))
        .arg("lint")
        .arg(assets_dir().join("check_dummy_hit"))
        .arg("some_1.0_all.changes")
        .output()
        .expect("spawning debmagic");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Source tree"), "stderr: {stderr}");
}

fn assert_finished(stdout: &str, rules: usize) {
    let rule_word = if rules == 1 { "rule" } else { "rules" };
    let line = stdout
        .lines()
        .find(|line| line.starts_with("Finished in "))
        .unwrap_or("");
    let expected = format!(" with {rules} {rule_word} using ");
    assert!(
        line.contains(&expected) && (line.ends_with(" thread.") || line.ends_with(" threads.")),
        "expected finished report ({expected}), stdout: {stdout}"
    );
    assert_eq!(
        stdout.lines().next_back(),
        Some(line),
        "finished report should be the last line, stdout: {stdout}"
    );
    let time = line
        .strip_prefix("Finished in ")
        .and_then(|rest| rest.split(" with ").next())
        .unwrap_or("");
    assert!(
        time.ends_with("ms") || (time.ends_with('s') && time.contains('.')),
        "unexpected duration {time:?} in {line}"
    );
}

#[test]
fn parse_parity_hint_and_diagnostic_lines() {
    let parsed = parse_universal_hint_line(
        "generic-empty (source): required-field (in section for source) Standards-Version [debian/control:1]",
    );
    assert_eq!(
        parsed,
        Some((
            "source".to_string(),
            ParityHint {
                tag: "required-field".to_string(),
                extra: "(in section for source) Standards-Version".to_string(),
            }
        ))
    );

    let diagnostic = parse_diagnostic_line(
        "E: generic-empty (source): required-field (LN0001) (in section for source) Standards-Version [debian/control]",
    );
    assert_eq!(
        diagnostic,
        Some((
            "source".to_string(),
            ParityHint {
                tag: "required-field".to_string(),
                extra: "(in section for source) Standards-Version".to_string(),
            }
        ))
    );
}

#[test]
fn parse_overridden_line_is_separate_from_diagnostic_lines() {
    let line = "O: generic-empty (source): required-field (LN0001) (in section for source) Standards-Version [debian/control]";
    let expected = Some((
        "source".to_string(),
        ParityHint {
            tag: "required-field".to_string(),
            extra: "(in section for source) Standards-Version".to_string(),
        },
    ));
    assert_eq!(parse_overridden_line(line), expected);
    assert_eq!(parse_diagnostic_line(line), None);
    let plain = line.replacen("O:", "W:", 1);
    assert_eq!(parse_diagnostic_line(&plain), expected);
    assert_eq!(parse_overridden_line(&plain), None);
    assert_eq!(parse_overridden_line("N: some justification"), None);
}

#[test]
fn missing_upstream_tarball_uses_gz_from_brace_list() {
    let expected = Some("cruft-general-upstream_1.0.orig.tar.gz");
    let cwd_relative = "dpkg-source: error: cannot build with source format '3.0 (quilt)': \
         no upstream tarball found at ./cruft-general-upstream_1.0.orig.tar.{bz2,gz,lzma,xz}";
    assert_eq!(missing_upstream_tarball(cwd_relative).as_deref(), expected);
    // dpkg-source -b src (parent cwd) prints a path relative to the tree.
    let tree_relative = "dpkg-source: error: can't build with source format '3.0 (quilt)': \
         no upstream tarball found at ../cruft-general-upstream_1.0.orig.tar.{bz2,gz,lzma,xz}";
    assert_eq!(missing_upstream_tarball(tree_relative).as_deref(), expected);
    assert!(missing_upstream_tarball("dpkg-source: error: cannot stat directory src").is_none());
}

/// The Diagnostic `cute-field` reports for a `HOMEPAGE` field in
/// `debian/control`, as a line ends (the processable part comes before it).
const CUTE_FIELD_END: &str = "cute-field (LN1370) HOMEPAGE vs Homepage [debian/control:3]";

/// A Source tree whose `debian/control` has a `HOMEPAGE` field (a `cute-field`
/// Diagnostic), with `overrides` as its `debian/source/lintian-overrides`.
fn cute_field_tree(work: &Path, overrides: Option<&str>) -> PathBuf {
    let tree = work.join("src");
    fs::create_dir_all(tree.join("debian/source")).expect("debian/source");
    fs::write(
        tree.join("debian/control"),
        "Source: example\nMaintainer: Example <ex@example.com>\nHOMEPAGE: https://example.org/\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
    )
    .expect("control");
    if let Some(text) = overrides {
        fs::write(tree.join("debian/source/lintian-overrides"), text).expect("overrides");
    }
    tree
}

fn printed_cute_field(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| line.ends_with(CUTE_FIELD_END))
        .collect()
}

#[test]
fn lint_override_hides_matching_diagnostic_by_default() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("cute-field HOMEPAGE*\n"));
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(printed_cute_field(&stdout).is_empty(), "stdout: {stdout}");
    assert!(stdout.contains("All checks passed!"), "stdout: {stdout}");
    assert!(stdout.contains("N: 1 hint overridden"), "stdout: {stdout}");
    assert_finished(&stdout, 1);
}

#[test]
fn lint_show_overrides_prints_the_justification_and_an_o_line() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(
        work.path(),
        Some("# the maintainer chose this\ncute-field HOMEPAGE*\n"),
    );
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field", "--show-overrides"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    let lines: Vec<&str> = stdout.lines().collect();
    let shown = lines
        .iter()
        .position(|line| line.starts_with("O:") && line.ends_with(CUTE_FIELD_END))
        .unwrap_or_else(|| panic!("no O: line, stdout: {stdout}"));
    assert_eq!(lines[shown - 1], "N: the maintainer chose this", "{stdout}");
    // Overridden hints do not count in the summary or the note.
    assert!(stdout.contains("All checks passed!"), "stdout: {stdout}");
    assert!(!stdout.contains("overridden"), "stdout: {stdout}");
}

#[test]
fn lint_no_override_prints_the_diagnostic_and_no_note() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("cute-field HOMEPAGE*\n"));
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field", "--no-override"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert_eq!(printed_cute_field(&stdout).len(), 1, "stdout: {stdout}");
    assert!(
        stdout.lines().all(|line| !line.starts_with("N:")),
        "stdout: {stdout}"
    );
}

#[test]
fn lint_override_pattern_must_glob_the_context() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("cute-field X-Dh*\n"));
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field"]);

    assert_eq!(status.code(), Some(0), "stderr: {stderr}\nstdout: {stdout}");
    assert_eq!(printed_cute_field(&stdout).len(), 1, "stdout: {stdout}");
    assert!(!stdout.contains("overridden"), "stdout: {stdout}");
}

#[test]
fn lint_override_architecture_list_is_matched_against_source() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("[amd64]: cute-field HOMEPAGE*\n"));
    let (_status, stdout, _stderr) = run_lint(&tree, &["--select", "cute-field"]);
    assert_eq!(printed_cute_field(&stdout).len(), 1, "stdout: {stdout}");

    let tree = cute_field_tree(work.path(), Some("[source]: cute-field HOMEPAGE*\n"));
    let (_status, stdout, _stderr) = run_lint(&tree, &["--select", "cute-field"]);
    assert!(printed_cute_field(&stdout).is_empty(), "stdout: {stdout}");
}

#[test]
fn lint_override_durable_tag_is_ignored_with_a_note() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(
        work.path(),
        Some("malformed-override\ncute-field HOMEPAGE*\n"),
    );
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field,malformed-override"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(printed_cute_field(&stdout).is_empty(), "stdout: {stdout}");
    assert!(
        stdout.contains("N: Some overrides were ignored."),
        "stdout: {stdout}"
    );
}

#[test]
fn lint_override_for_a_rule_that_did_not_run_is_left_alone() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("package-installs-apt-sources\n"));
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert_eq!(printed_cute_field(&stdout).len(), 1, "stdout: {stdout}");
    assert!(
        stdout.lines().all(|line| !line.starts_with("N:")),
        "stdout: {stdout}"
    );
}

/// A binary package with the given installed files and override file, built
/// with `dpkg-deb`.
fn build_deb(work: &Path, arch: &str, overrides: Option<&str>) -> PathBuf {
    let root = work.join("root");
    fs::create_dir_all(root.join("DEBIAN")).expect("DEBIAN");
    fs::write(
        root.join("DEBIAN/control"),
        format!(
            "Package: example\nVersion: 1.0\nArchitecture: {arch}\nMaintainer: Example <ex@example.com>\nDescription: example\n extra\n"
        ),
    )
    .expect("control");
    let sources = root.join("etc/apt");
    fs::create_dir_all(&sources).expect("etc/apt");
    fs::write(
        sources.join("sources.list"),
        "deb http://example.org/ stable main\n",
    )
    .expect("sources.list");
    if let Some(text) = overrides {
        let dir = root.join("usr/share/lintian/overrides");
        fs::create_dir_all(&dir).expect("overrides dir");
        fs::write(dir.join("example"), text).expect("override file");
    }
    let deb = work.join(format!("example_1.0_{arch}.deb"));
    let output = Command::new("dpkg-deb")
        .args(["--root-owner-group", "--build"])
        .arg(&root)
        .arg(&deb)
        .output()
        .expect("spawning dpkg-deb");
    assert!(
        output.status.success(),
        "dpkg-deb failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    deb
}

#[test]
fn lint_binary_override_applies_to_its_installable() {
    let work = tempfile::tempdir().expect("workdir");
    let deb = build_deb(
        work.path(),
        "amd64",
        Some("[amd64]: package-installs-apt-sources [etc/apt/sources.list]\n"),
    );
    let (status, stdout, stderr) =
        run_lint_subject(&deb, &["--select", "package-installs-apt-sources"]);

    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(
        !stdout.contains("package-installs-apt-sources (LN1003)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("N: 1 hint overridden"), "stdout: {stdout}");

    let (_status, stdout, _stderr) = run_lint_subject(
        &deb,
        &[
            "--select",
            "package-installs-apt-sources",
            "--show-overrides",
        ],
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("O:")
                && line.contains("package-installs-apt-sources (LN1003)")),
        "stdout: {stdout}"
    );
}

#[test]
fn lint_binary_override_for_another_architecture_does_not_apply() {
    let work = tempfile::tempdir().expect("workdir");
    let deb = build_deb(
        work.path(),
        "amd64",
        Some("[arm64]: package-installs-apt-sources [etc/apt/sources.list]\n"),
    );
    let (status, stdout, stderr) =
        run_lint_subject(&deb, &["--select", "package-installs-apt-sources"]);
    // An error Diagnostic, so the policy failure is expected.
    assert_eq!(status.code(), Some(1), "stderr: {stderr}\nstdout: {stdout}");
    assert!(
        stdout.contains("package-installs-apt-sources (LN1003)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("overridden"), "stdout: {stdout}");
}

/// A Source tree whose `debian/control` has no `HOMEPAGE` field, so `cute-field`
/// makes no Diagnostic, with `overrides` as its override file.
fn cute_field_free_tree(work: &Path, overrides: Option<&str>) -> PathBuf {
    let tree = work.join("src");
    fs::create_dir_all(tree.join("debian/source")).expect("debian/source");
    fs::write(
        tree.join("debian/control"),
        "Source: example\nMaintainer: Example <ex@example.com>\nHomepage: https://example.org/\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
    )
    .expect("control");
    if let Some(text) = overrides {
        fs::write(tree.join("debian/source/lintian-overrides"), text).expect("overrides");
    }
    tree
}

#[test]
fn lint_mismatched_override_is_reported_by_default() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("cute-field nope\n"));
    // `--select` replaces the default set, so the default-selected Tag is named.
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field,mismatched-override"]);

    // A warning: printed, and not a policy failure under the default FailOn.
    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(
        stdout.contains(
            "W: example (source): mismatched-override (LN1780) cute-field nope [debian/source/lintian-overrides:1]"
        ),
        "stdout: {stdout}"
    );
    assert_eq!(printed_cute_field(&stdout).len(), 1, "stdout: {stdout}");
    assert!(stdout.contains("N: 1 unused override"), "stdout: {stdout}");
}

#[test]
fn lint_unused_override_needs_its_selection() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_free_tree(work.path(), Some("cute-field\n"));

    // Not selected by default: lintian's info Tags are not shown unless enabled.
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field"]);
    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(!stdout.contains("unused-override"), "stdout: {stdout}");
    assert!(!stdout.contains("N:"), "stdout: {stdout}");

    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field,unused-override"]);
    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(
        stdout.contains(
            "I: example (source): unused-override (LN1779) cute-field  [debian/source/lintian-overrides:1]"
        ),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("N: 1 unused override"), "stdout: {stdout}");
}

#[test]
fn lint_override_rules_respect_no_override_and_show_overrides() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_tree(work.path(), Some("cute-field nope\n"));
    let (_status, stdout, stderr) = run_lint(
        &tree,
        &[
            "--select",
            "cute-field,mismatched-override",
            "--no-override",
        ],
    );
    assert!(
        !stdout.contains("mismatched-override"),
        "stdout: {stdout}\nstderr: {stderr}"
    );

    // Under --show-overrides the note on what was overridden is left out; the
    // reports themselves are not overridden and still print.
    let (_status, stdout, _stderr) = run_lint(
        &tree,
        &[
            "--select",
            "cute-field,mismatched-override",
            "--show-overrides",
        ],
    );
    assert!(
        stdout.contains("mismatched-override (LN1780)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("unused override"), "stdout: {stdout}");
}

#[test]
fn lint_selected_unused_override_without_an_override_file_runs_and_reports_nothing() {
    let work = tempfile::tempdir().expect("workdir");
    let tree = cute_field_free_tree(work.path(), None);
    let (status, stdout, stderr) = run_lint(&tree, &["--select", "cute-field,unused-override"]);
    // The Rule applies to the Source tree; with no file it has nothing to report.
    assert!(status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(!stdout.contains("unused-override"), "stdout: {stdout}");
    assert!(stdout.contains("All checks passed!"), "stdout: {stdout}");
}
