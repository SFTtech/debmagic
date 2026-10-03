//! End-to-end test for signing: builds a throwaway gpg key with a loopback
//! pinentry, crafts a minimal source package artifact set (`.dsc` +
//! `.changes`), then signs it with `debmagic sign` and verifies the result
//! with gpg.
//!
//! Requires gpg on the host. Ignored by default; run with:
//!
//! ```shell
//! cargo test --test signing -- --ignored --nocapture
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Run `cmd`, panicking with stdout+stderr on failure.
fn run(cmd: &mut Command) -> String {
    let output = cmd.output().expect("failed to spawn command");
    if !output.status.success() {
        panic!(
            "command failed: {:?}\nstdout: {}\nstderr: {}",
            cmd,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

struct TestGpgHome {
    dir: PathBuf,
}

impl TestGpgHome {
    /// Create an isolated GNUPGHOME with a throwaway signing key and an
    /// agent that answers without pinentry.
    fn create() -> Self {
        let dir = std::env::temp_dir().join(format!("debmagic-sign-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        // gpg refuses to use a homedir others could access.
        run(Command::new("chmod").args(["700"]).arg(&dir));

        fs::write(dir.join("gpg-agent.conf"), "allow-loopback-pinentry\n").unwrap();
        fs::write(dir.join("gpg.conf"), "pinentry-mode loopback\n").unwrap();

        run(Command::new("gpgconf")
            .env("GNUPGHOME", &dir)
            .args(["--launch", "gpg-agent"]));

        run(Command::new("gpg").env("GNUPGHOME", &dir).args([
            "--batch",
            "--passphrase",
            "",
            "--quick-generate-key",
            "debmagic sign test <sign@example.invalid>",
            "ed25519",
            "sign",
            "never",
        ]));

        Self { dir }
    }

    /// Add a signing subkey, which gpg then prefers over the primary key —
    /// the usual layout of real-world keys.
    fn add_signing_subkey(&self) {
        let listing = run(Command::new("gpg").env("GNUPGHOME", &self.dir).args([
            "--list-keys",
            "--with-colons",
            "sign@example.invalid",
        ]));
        let fingerprint = listing
            .lines()
            .find_map(|line| line.strip_prefix("fpr:"))
            .and_then(|rest| rest.split(':').nth(8))
            .expect("the test key has a fingerprint");
        run(Command::new("gpg").env("GNUPGHOME", &self.dir).args([
            "--batch",
            "--passphrase",
            "",
            "--quick-add-key",
            fingerprint,
            "ed25519",
            "sign",
            "never",
        ]));
    }
}

impl Drop for TestGpgHome {
    fn drop(&mut self) {
        let _ = Command::new("gpgconf")
            .env("GNUPGHOME", &self.dir)
            .args(["--kill", "all"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn sha256(data: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn sha256sum");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(data)
        .expect("failed to pipe to sha256sum");
    let out = child.wait_with_output().expect("sha256sum failed");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// Write a minimal artifact set (`hello.txt`, `.dsc`, `.changes`) with
/// consistent sizes and checksums.
fn write_fake_artifacts(output_dir: &Path) -> PathBuf {
    fs::create_dir_all(output_dir).unwrap();
    let payload = b"hello from debmagic sign test\n";
    fs::write(output_dir.join("hello.txt"), payload).unwrap();

    let files_entry = |name: &str, data: &[u8]| {
        format!(
            " {} {} {} {} {}",
            sha256(data),
            data.len(),
            "x",
            "optional",
            name
        )
    };

    let dsc_content = format!(
        "Format: 3.0 (native)\nSource: debmagic-sign-test\nBinary: debmagic-sign-test\nVersion: 1.0\nMaintainer: debmagic sign test <sign@example.invalid>\nArchitecture: all\nFiles:\n{}\n",
        files_entry("hello.txt", payload)
    );
    let dsc_name = "debmagic-sign-test_1.0.dsc";
    fs::write(output_dir.join(dsc_name), &dsc_content).unwrap();

    let changes_content = format!(
        "Format: 1.8\nSource: debmagic-sign-test\nBinary: debmagic-sign-test\nVersion: 1.0\nMaintainer: debmagic sign test <sign@example.invalid>\nChanged-By: debmagic sign test <sign@example.invalid>\nArchitecture: source all\nDistribution: unstable\nFiles:\n{}\n{}\n",
        files_entry(dsc_name, dsc_content.as_bytes()),
        files_entry("hello.txt", payload)
    );
    let changes_path = output_dir.join("debmagic-sign-test_1.0_amd64.changes");
    fs::write(&changes_path, &changes_content).unwrap();
    changes_path
}

#[test]
#[ignore = "needs gpg on the host"]
fn signs_changes_and_dsc_and_rewrites_checksums() {
    let gpg_home = TestGpgHome::create();
    let work_dir = std::env::temp_dir().join(format!("debmagic-sign-out-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    // Hermetic: ignore the user's global config and pin the key explicitly,
    // like debsign's maintainer lookup would resolve it.
    let bin = env!("CARGO_BIN_EXE_debmagic");
    run(Command::new(bin)
        .env("GNUPGHOME", &gpg_home.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args(["sign", "--sign-key", "sign@example.invalid"])
        .arg(&changes_path));

    let signed = fs::read_to_string(&changes_path).unwrap();
    assert!(
        signed.contains("-----BEGIN PGP SIGNATURE-----"),
        "changes file was not signed:\n{signed}"
    );
    let dsc_path = work_dir.join("debmagic-sign-test_1.0.dsc");
    let signed_dsc = fs::read_to_string(&dsc_path).unwrap();
    assert!(
        signed_dsc.contains("-----BEGIN PGP SIGNATURE-----"),
        "dsc file was not signed:\n{signed_dsc}"
    );

    // The .changes checksums must now match the signed .dsc.
    let dsc_data = fs::read(&dsc_path).unwrap();
    let md5 = {
        let mut child = Command::new("md5sum")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(&dsc_data).unwrap();
        let out = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    };
    assert!(
        signed.contains(&format!(
            " {md5} {} x optional debmagic-sign-test_1.0.dsc",
            dsc_data.len()
        )),
        "changes checksums were not rewritten for the signed dsc:\n{signed}"
    );

    // Verify both signatures against the test keyring.
    run(Command::new("gpg")
        .env("GNUPGHOME", &gpg_home.dir)
        .args(["--batch", "--verify"])
        .arg(&changes_path));
    run(Command::new("gpg")
        .env("GNUPGHOME", &gpg_home.dir)
        .args(["--batch", "--verify"])
        .arg(&dsc_path));

    let _ = fs::remove_dir_all(&work_dir);
}

#[test]
#[ignore = "needs gpg on the host"]
fn auto_skips_same_key_and_force_resigns() {
    let gpg_home = TestGpgHome::create();
    let work_dir =
        std::env::temp_dir().join(format!("debmagic-sign-auto-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    let bin = env!("CARGO_BIN_EXE_debmagic");
    let sign = |args: &[&str]| {
        run(Command::new(bin)
            .env("GNUPGHOME", &gpg_home.dir)
            .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
            .args(["sign", "--sign-key", "sign@example.invalid"])
            .args(args)
            .arg(&changes_path))
    };

    // First sign (auto): everything gets signed.
    sign(&["--mode", "auto"]);
    let first = fs::read_to_string(&changes_path).unwrap();
    assert!(first.contains("-----BEGIN PGP SIGNATURE-----"));

    // Second sign (auto): same key already signed it, so the file is
    // left byte-for-byte alone.
    let output = sign(&["--mode", "auto"]);
    let second = fs::read_to_string(&changes_path).unwrap();
    assert_eq!(first, second, "auto re-signed a same-key signature");
    assert!(
        output.contains("already signed with this key; skipping"),
        "auto did not detect the same-key signature:\n{output}"
    );

    // Force: the file is re-signed (mtime changes even if the armor
    // happens to be identical) and still verifies.
    let mtime_before = fs::metadata(&changes_path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    sign(&["--mode", "force"]);
    let mtime_after = fs::metadata(&changes_path).unwrap().modified().unwrap();
    assert!(mtime_after > mtime_before, "force did not rewrite the file");
    run(Command::new("gpg")
        .env("GNUPGHOME", &gpg_home.dir)
        .args(["--batch", "--verify"])
        .arg(&changes_path));

    let _ = fs::remove_dir_all(&work_dir);
}

#[test]
#[ignore = "needs gpg on the host"]
fn auto_skips_same_key_signed_by_subkey() {
    let gpg_home = TestGpgHome::create();
    gpg_home.add_signing_subkey();
    let work_dir =
        std::env::temp_dir().join(format!("debmagic-sign-subkey-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    let bin = env!("CARGO_BIN_EXE_debmagic");
    let sign = || {
        run(Command::new(bin)
            .env("GNUPGHOME", &gpg_home.dir)
            .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
            .args([
                "sign",
                "--sign-key",
                "sign@example.invalid",
                "--mode",
                "auto",
            ])
            .arg(&changes_path))
    };

    sign();
    // deterministic ed25519 signatures within the same second are
    // byte-identical, so only the reported decision proves the skip
    let output = sign();
    assert!(
        output.contains("already signed with this key; skipping"),
        "auto re-signed a signature made by our subkey:\n{output}"
    );

    let _ = fs::remove_dir_all(&work_dir);
}

#[test]
#[ignore = "needs gpg on the host"]
fn auto_resigns_a_different_key() {
    let ours = TestGpgHome::create();
    let theirs = TestGpgHome::create();
    let work_dir =
        std::env::temp_dir().join(format!("debmagic-sign-other-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    // Sign with a second, different key.
    let bin = env!("CARGO_BIN_EXE_debmagic");
    run(Command::new(bin)
        .env("GNUPGHOME", &theirs.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args([
            "sign",
            "--sign-key",
            "sign@example.invalid",
            "--mode",
            "force",
        ])
        .arg(&changes_path));
    let foreign = fs::read_to_string(&changes_path).unwrap();
    assert!(foreign.contains("-----BEGIN PGP SIGNATURE-----"));

    // auto with our key must detect the foreign signature and re-sign.
    run(Command::new(bin)
        .env("GNUPGHOME", &ours.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args([
            "sign",
            "--sign-key",
            "sign@example.invalid",
            "--mode",
            "auto",
        ])
        .arg(&changes_path));
    let resigned = fs::read_to_string(&changes_path).unwrap();
    assert_ne!(foreign, resigned, "auto kept a different key's signature");
    run(Command::new("gpg")
        .env("GNUPGHOME", &ours.dir)
        .args(["--batch", "--verify"])
        .arg(&changes_path));

    let _ = fs::remove_dir_all(&work_dir);
}

#[test]
#[ignore = "needs gpg on the host"]
fn keep_accepts_a_foreign_signature() {
    let ours = TestGpgHome::create();
    let theirs = TestGpgHome::create();
    let work_dir =
        std::env::temp_dir().join(format!("debmagic-sign-keep-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    // Sign with a second, different key.
    let bin = env!("CARGO_BIN_EXE_debmagic");
    run(Command::new(bin)
        .env("GNUPGHOME", &theirs.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args([
            "sign",
            "--sign-key",
            "sign@example.invalid",
            "--mode",
            "force",
        ])
        .arg(&changes_path));
    let foreign = fs::read_to_string(&changes_path).unwrap();
    assert!(foreign.contains("-----BEGIN PGP SIGNATURE-----"));

    // keep must leave the foreign signature byte-for-byte alone.
    run(Command::new(bin)
        .env("GNUPGHOME", &ours.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args([
            "sign",
            "--sign-key",
            "sign@example.invalid",
            "--mode",
            "keep",
        ])
        .arg(&changes_path));
    let kept = fs::read_to_string(&changes_path).unwrap();
    assert_eq!(foreign, kept, "keep re-signed a foreign signature");

    // keep still signs an unsigned file.
    let unsigned_dir =
        std::env::temp_dir().join(format!("debmagic-sign-keep2-{}", uuid::Uuid::new_v4()));
    let unsigned_path = write_fake_artifacts(&unsigned_dir);
    run(Command::new(bin)
        .env("GNUPGHOME", &ours.dir)
        .env("DEBMAGIC_CONFIG_GLOBAL", "/dev/null")
        .args([
            "sign",
            "--sign-key",
            "sign@example.invalid",
            "--mode",
            "keep",
        ])
        .arg(&unsigned_path));
    let signed = fs::read_to_string(&unsigned_path).unwrap();
    assert!(
        signed.contains("-----BEGIN PGP SIGNATURE-----"),
        "keep did not sign an unsigned file:\n{signed}"
    );

    let _ = fs::remove_dir_all(&work_dir);
    let _ = fs::remove_dir_all(&unsigned_dir);
}
