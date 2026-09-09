//! Integration tests for `debmagic lint` via the built binary.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{
    ParityHint, assets_dir, build_native_dsc, missing_upstream_tarball, parse_diagnostic_line,
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
