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
