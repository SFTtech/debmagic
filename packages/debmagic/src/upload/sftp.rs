use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use anyhow::Context;

use super::{UploadTarget, Uploader};
use crate::subprocess::{self, Capture};

pub struct SftpUploader {
    pub target: UploadTarget,
}

impl Uploader for SftpUploader {
    fn upload_files(&mut self, files: &[PathBuf]) -> anyhow::Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        let host = self.target.host_string();
        // sftp has no `~` expansion; a path starting with `~user` is
        // relative to that user's home on the server, which is exactly
        // what the ppa builtin's `~{target}/ubuntu` relies on.
        let incoming = self.target.incoming_dir();

        // one batch of `put` commands uploads everything in a single
        // sftp session instead of one connection per file
        let mut batch = String::new();
        for file in files {
            let name = file
                .file_name()
                .context("upload file has no file name")?
                .to_string_lossy();
            batch.push_str(&format!(
                "put \"{}\" \"{incoming}/{name}\"\n",
                file.display()
            ));
        }
        batch.push_str("pwd\n");

        let mut cmd = Command::new("sftp");
        cmd.arg("-b")
            .arg("-")
            .args(self.target.hostkey_args())
            .args(self.target.port_args())
            .arg(&host)
            // sftp forks ssh for the connection; a process group lets us
            // tear down both instead of orphaning the ssh child
            .process_group(0);

        let mut child = subprocess::command(cmd)
            .input(batch.as_bytes())
            .capture(Capture::STDOUT)
            .spawn()
            .with_context(|| format!("starting sftp for {host}:{incoming} failed"))?;

        // stream the session output as it happens; each `sftp> put` echo
        // marks a file being sent, the final `pwd` output means all puts
        // succeeded and the server will never close the connection, so stop
        // waiting and tear the session down
        let mut stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut done = false;
        let mut line = String::new();
        loop {
            line.clear();
            match stdout.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Some(path) = line
                        .strip_prefix("sftp> put \"")
                        .and_then(|rest| rest.split('"').next())
                    {
                        let index = files
                            .iter()
                            .position(|f| f.as_os_str() == path)
                            .map(|i| i + 1)
                            .unwrap_or_default();
                        let name = Path::new(path)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.to_string());
                        println!("debmagic: uploading {name} ({index}/{})", files.len());
                    }
                    if line.contains("Remote working directory:") {
                        done = true;
                        break;
                    }
                }
            }
        }
        if !done {
            // the batch aborted before the sentinel: let sftp finish so
            // the exit status carries the real error; stderr already
            // reached the user's terminal
            let status = child
                .wait()
                .with_context(|| format!("waiting for sftp to {host}:{incoming} failed"))?;
            anyhow::bail!(
                "sftp failed (exit status: {status}) uploading {} files to {host}:{incoming}",
                files.len()
            );
        }
        terminate_session(&mut child, &host, &incoming)?;
        Ok(())
    }
}

/// Kill the sftp process group; the server will not close the connection
/// on its own, so this is the only way to end the session.
fn terminate_session(child: &mut Child, host: &str, incoming: &str) -> anyhow::Result<()> {
    // SIGTERM to the group lets sftp and its ssh child close the channel
    // cleanly; the id is ours because of `process_group(0)`
    unsafe { libc::kill(-(child.id() as i32), libc::SIGTERM) };
    child
        .wait()
        .with_context(|| format!("terminating the sftp session for {host}:{incoming} failed"))?;
    Ok(())
}
