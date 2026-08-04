//! End-to-end test for container signing: builds a throwaway gpg key with a
//! loopback pinentry, crafts a minimal source package artifact set
//! (`.dsc` + `.changes`), then has the docker driver sign it in a minimal
//! container via the forwarded gpg-agent socket.
//!
//! Requires docker and gpg on the host. Ignored by default; run with:
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

    fn agent_extra_socket(&self) -> PathBuf {
        let out = run(Command::new("gpgconf")
            .env("GNUPGHOME", &self.dir)
            .args(["--list-dirs", "agent-extra-socket"]));
        PathBuf::from(out.trim())
    }

    fn fingerprint(&self) -> String {
        let out = run(Command::new("gpg").env("GNUPGHOME", &self.dir).args([
            "--batch",
            "--with-colons",
            "--list-secret-keys",
            "sign@example.invalid",
        ]));
        for line in out.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.first() == Some(&"fpr") {
                return fields[9].to_string();
            }
        }
        panic!("no fingerprint found");
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

/// Write a minimal artifact set (`hello.txt`, `.dsc`, `.changes`) with
/// consistent sizes and sha256 checksums, so `debsign` accepts it.
fn write_fake_artifacts(output_dir: &Path) -> PathBuf {
    fs::create_dir_all(output_dir).unwrap();
    let payload = b"hello from debmagic sign test\n";
    fs::write(output_dir.join("hello.txt"), payload).unwrap();

    let sha256 = |data: &[u8]| -> String {
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
    };

    let files_entry =
        |name: &str, data: &[u8]| format!(" {} {} {}", sha256(data), data.len(), name);

    let dsc_content = format!(
        "Format: 3.0 (native)\nSource: debmagic-sign-test\nBinary: debmagic-sign-test\nVersion: 1.0\nMaintainer: debmagic sign test <sign@example.invalid>\nArchitecture: all\nFiles:\n{}\n",
        files_entry("hello.txt", payload)
    );
    let dsc_name = "debmagic-sign-test_1.0.dsc";
    fs::write(output_dir.join(dsc_name), &dsc_content).unwrap();

    let changes_content = format!(
        "Format: 1.8\nSource: debmagic-sign-test\nBinary: debmagic-sign-test\nVersion: 1.0\nMaintainer: debmagic sign test <sign@example.invalid>\nArchitecture: source all\nDistribution: unstable\nFiles:\n{}\n{}\n",
        files_entry(dsc_name, dsc_content.as_bytes()),
        files_entry("hello.txt", payload)
    );
    let changes_path = output_dir.join("debmagic-sign-test_1.0_amd64.changes");
    fs::write(&changes_path, &changes_content).unwrap();
    changes_path
}

#[test]
#[ignore = "needs docker and gpg on the host"]
fn docker_signs_changes_with_forwarded_agent() {
    // Skip early with a clear message if docker isn't usable.
    if Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        eprintln!("docker not available, skipping");
        return;
    }

    let gpg_home = TestGpgHome::create();
    let work_dir = std::env::temp_dir().join(format!("debmagic-sign-out-{}", uuid::Uuid::new_v4()));
    let changes_path = write_fake_artifacts(&work_dir);

    let output_dir = changes_path.parent().unwrap();
    let staging_dir = work_dir.join("staging");
    fs::create_dir_all(&staging_dir).unwrap();

    // Stage pubkey + ownertrust the same way the driver does. The export is
    // binary OpenPGP data, so capture raw bytes rather than a String.
    let pubkey = Command::new("gpg")
        .env("GNUPGHOME", &gpg_home.dir)
        .args(["--batch", "--export", "sign@example.invalid"])
        .output()
        .expect("gpg export failed");
    assert!(pubkey.status.success());
    fs::write(staging_dir.join("pubkey.asc"), pubkey.stdout).unwrap();
    fs::write(
        staging_dir.join("ownertrust.txt"),
        format!("{}:6:\n", gpg_home.fingerprint()),
    )
    .unwrap();

    let socket = gpg_home.agent_extra_socket();
    let script = "set -e; \
         export GNUPGHOME=/root/.gnupg; \
         mkdir -p /run/user/0/gnupg \"$GNUPGHOME\"; \
         chmod 700 /run/user/0/gnupg \"$GNUPGHOME\"; \
         ln -sf /tmp/debmagic-gpg/S.gpg-agent /run/user/0/gnupg/S.gpg-agent; \
         ln -sf /tmp/debmagic-gpg/S.gpg-agent \"$GNUPGHOME/S.gpg-agent\"; \
         apt-get update -qq; \
         apt-get install -y -qq devscripts; \
         gpg --batch --import /debmagic-sign/pubkey.asc; \
         gpg --batch --import-ownertrust /debmagic-sign/ownertrust.txt; \
         cd /debmagic-output && debsign -k'sign@example.invalid' 'debmagic-sign-test_1.0_amd64.changes'";

    run(Command::new("docker")
        .args(["run", "--rm", "--init"])
        .args([
            "--mount",
            &format!(
                "type=bind,src={},dst=/debmagic-sign,readonly",
                staging_dir.display()
            ),
        ])
        .arg(format!(
            "--mount=type=bind,src={},dst=/tmp/debmagic-gpg/S.gpg-agent,readonly",
            socket.display()
        ))
        .args([
            "--mount",
            &format!(
                "type=bind,src={},dst=/debmagic-output",
                output_dir.display()
            ),
        ])
        .arg("docker.io/debian:trixie")
        .args(["sh", "-ec", script]));

    let signed = fs::read_to_string(&changes_path).unwrap();
    assert!(
        signed.contains("-----BEGIN PGP SIGNATURE-----"),
        "changes file was not signed:\n{signed}"
    );

    // Verify the signature against the test keyring. The .changes file has
    // the message and signature in one clearsigned document.
    run(Command::new("gpg")
        .env("GNUPGHOME", &gpg_home.dir)
        .args(["--batch", "--verify"])
        .arg(&changes_path));

    let _ = fs::remove_dir_all(&work_dir);
}
