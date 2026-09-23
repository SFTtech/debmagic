use super::UploadTarget;

/// Shared ssh-based transport bits for the scp/sftp uploaders.
impl UploadTarget {
    /// `login@server`, or just `server` when no login is configured
    /// (the ssh config then decides the user).
    pub fn host_string(&self) -> String {
        match &self.config.login {
            Some(login) => format!("{login}@{}", self.config.server),
            None => self.config.server.clone(),
        }
    }

    /// `["-P", "<port>"]` — scp and sftp both use `-P` for the port;
    /// empty when unconfigured.
    pub fn port_args(&self) -> Vec<String> {
        match self.config.port {
            Some(port) => vec!["-P".to_string(), port.to_string()],
            None => Vec::new(),
        }
    }

    /// `["-oStrictHostKeyChecking=accept-new"]` when TOFU host-key
    /// trust is enabled for this target, else empty. sftp batch mode
    /// cannot prompt for an unknown host key, so without this a first
    /// upload to a new host fails with a bare "Host key verification
    /// failed".
    pub fn hostkey_args(&self) -> Vec<String> {
        if self.config.tofu_hostkey {
            vec!["-oStrictHostKeyChecking=accept-new".to_string()]
        } else {
            Vec::new()
        }
    }
}
