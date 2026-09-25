use std::path::PathBuf;

use anyhow::Context;

use super::{UploadTarget, Uploader};

pub struct ScpUploader {
    pub target: UploadTarget,
}

impl Uploader for ScpUploader {
    fn upload_files(&mut self, files: &[PathBuf]) -> anyhow::Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        let host = self.target.host_string();
        let incoming = self.target.incoming_dir();
        let destination = format!("{host}:{incoming}/");
        let mut command = std::process::Command::new("scp");
        command
            .arg("-p")
            .args(self.target.hostkey_args())
            .args(self.target.port_args());
        for file in files {
            command.arg(file);
        }
        command.arg(&destination);
        let status = command
            .status()
            .with_context(|| format!("starting scp for {destination} failed"))?;
        if !status.success() {
            anyhow::bail!("scp failed (exit status: {status}) uploading to {destination}");
        }
        Ok(())
    }
}
