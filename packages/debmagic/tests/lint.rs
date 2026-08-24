//! Integration tests for `debmagic lint` via the built binary.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets")
}

fn run_lint(source_dir: &Path, extra_args: &[&str]) -> (ExitStatus, String, String) {
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

fn run_lint_subject(subject: &Path, extra_args: &[&str]) -> (ExitStatus, String, String) {
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

fn temp_artifact(extension: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "debmagic-lint-integration-{}.{extension}",
        std::process::id()
    ));
    fs::write(&path, b"").expect("writing temp artifact");
    path
}

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
}

#[test]
fn lint_required_field_miss_prints_error_and_fails() {
    let source_dir = assets_dir().join("check_required_field_miss");
    let (status, stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(
        !status.success(),
        "expected policy failure, stderr: {stderr}"
    );
    assert_eq!(status.code(), Some(1));
    assert!(stdout.contains("LN0001"), "stdout: {stdout}");
    assert!(stdout.contains("required-field"), "stdout: {stdout}");
    assert!(stdout.contains("Maintainer"), "stdout: {stdout}");
    assert!(
        stdout.contains("Found 1 error."),
        "expected ruff-style summary, stdout: {stdout}"
    );
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
}

#[test]
fn lint_binary_package_via_positional_fails_unimplemented() {
    let deb = temp_artifact("deb");
    let (status, _stdout, stderr) = run_lint_subject(&deb, &[]);
    let _ = fs::remove_file(&deb);

    assert!(!status.success(), "expected failure for .deb Subject");
    assert!(
        stderr.contains("Binary package") || stderr.contains("not implemented"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_binary_package_via_source_dir_fails_unimplemented() {
    let deb = temp_artifact("deb");
    let (status, _stdout, stderr) = run_lint(&deb, &[]);
    let _ = fs::remove_file(&deb);

    assert!(!status.success(), "expected failure for .deb Subject");
    assert!(
        stderr.contains("Binary package") || stderr.contains("not implemented"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_source_package_via_positional_fails_unimplemented() {
    let dsc = temp_artifact("dsc");
    let (status, _stdout, stderr) = run_lint_subject(&dsc, &[]);
    let _ = fs::remove_file(&dsc);

    assert!(!status.success(), "expected failure for .dsc Subject");
    assert!(
        stderr.contains("Source package") || stderr.contains("not implemented"),
        "stderr: {stderr}"
    );
}

#[test]
fn lint_source_package_via_source_dir_fails_unimplemented() {
    let dsc = temp_artifact("dsc");
    let (status, _stdout, stderr) = run_lint(&dsc, &[]);
    let _ = fs::remove_file(&dsc);

    assert!(!status.success(), "expected failure for .dsc Subject");
    assert!(
        stderr.contains("Source package") || stderr.contains("not implemented"),
        "stderr: {stderr}"
    );
}
