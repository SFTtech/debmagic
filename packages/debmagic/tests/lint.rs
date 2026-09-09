//! Integration tests for `debmagic lint` via the built binary.

mod common;

use std::fs;

use common::{
    ParityHint, assets_dir, catalogued_ln_tags, parity_suite_root, parse_diagnostic_line,
    parse_universal_hint_line, run_lint, run_lint_subject, temp_artifact,
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
fn lint_changelog_syntax_error_prints_warning_and_succeeds() {
    let source_dir = assets_dir().join("check_changelog_syntax_error");
    let (status, stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(status.success(), "expected success, stderr: {stderr}");
    assert!(stdout.contains("LN0002"), "stdout: {stdout}");
    assert!(
        stdout.contains("syntax-error-in-debian-changelog"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("not a Debian changelog"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Found 1 warning."),
        "expected ruff-style summary, stdout: {stdout}"
    );
}

#[test]
fn lint_rules_missing_target_prints_error_and_fails() {
    let source_dir = assets_dir().join("check_rules_missing_target");
    let (status, stdout, stderr) = run_lint(&source_dir, &[]);

    assert!(
        !status.success(),
        "expected policy failure, stderr: {stderr}"
    );
    assert_eq!(status.code(), Some(1));
    assert!(stdout.contains("LN0003"), "stdout: {stdout}");
    assert!(
        stdout.contains("debian-rules-missing-required-target"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("build-arch"), "stdout: {stdout}");
    assert!(
        stdout.contains("Found 2 errors."),
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
