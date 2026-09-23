use std::collections::HashMap;

use anyhow::bail;

use super::UploadMethod;

/// A named upload target definition, builtin or from the config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UploadTargetConfig {
    /// Which upload method to use. `None` keeps a builtin's method
    /// (or defaults to scp for config-only targets).
    pub method: Option<UploadMethod>,
    /// Host to upload to.
    pub server: String,
    /// Remote directory to upload into; supports `{target}`.
    pub incoming: String,
    /// Login on the remote host. `None` lets the ssh config decide,
    /// then the local user — the Launchpad/Debian upload hosts expect
    /// your own username with a registered SSH key.
    pub login: Option<String>,
    /// Remote port. `None` lets the ssh config decide.
    pub port: Option<u16>,
    /// Trust the host key on first use (TOFU, ssh's `StrictHostKeyChecking=accept-new`).
    /// The upload is signed via pgp for the real trust.
    #[serde(default = "default_tofu_hostkey")]
    pub tofu_hostkey: bool,
    /// Commands to run before uploading, each via `sh -c`.
    pub pre_upload_commands: Vec<String>,
}

fn default_tofu_hostkey() -> bool {
    true
}

impl UploadTargetConfig {
    fn new(
        method: UploadMethod,
        server: &str,
        incoming: &str,
        login: Option<&str>,
        port: Option<u16>,
    ) -> Self {
        Self {
            method: Some(method),
            server: server.to_string(),
            incoming: incoming.to_string(),
            login: login.map(str::to_string),
            port,
            tofu_hostkey: true,
            pre_upload_commands: Vec::new(),
        }
    }
}

/// One resolved upload target.
#[derive(Debug, Clone)]
pub struct UploadTarget {
    pub name: String,
    /// The parameter after `:` in `ppa:user/repo`-style invocations.
    pub parameter: Option<String>,
    pub config: UploadTargetConfig,
}

impl UploadTarget {
    /// The remote directory to upload into, with `{target}` substituted.
    pub fn incoming_dir(&self) -> String {
        substitute_placeholders(
            &self.config.incoming,
            &[("target", self.parameter.as_deref().unwrap_or(""))],
        )
    }
}

/// Substitute `{name}` placeholders in `s`.
pub(crate) fn substitute_placeholders(s: &str, vars: &[(&str, &str)]) -> String {
    let mut out = s.to_string();
    for (name, value) in vars {
        let placeholder = format!("{{{name}}}");
        out = out.replace(&placeholder, value);
    }
    out
}

/// Parse `name` or `name:parameter` (split on the first `:`).
pub fn parse_target_spec(spec: &str) -> (String, Option<String>) {
    match spec.split_once(':') {
        Some((name, parameter)) => (name.to_string(), Some(parameter.to_string())),
        None => (spec.to_string(), None),
    }
}

/// Built-in upload targets. Each can be overridden/extended
/// field-by-field via `[upload.targets.<name>]` in the config file.
pub fn builtin_targets() -> Vec<(&'static str, UploadTargetConfig)> {
    vec![
        (
            "ppa",
            UploadTargetConfig::new(
                UploadMethod::Sftp,
                "ppa.launchpad.net",
                "~{target}/ubuntu",
                None,
                None,
            ),
        ),
        (
            "ubuntu",
            UploadTargetConfig::new(
                UploadMethod::Sftp,
                "upload.ubuntu.com",
                "ubuntu",
                None,
                None,
            ),
        ),
        (
            "debian",
            UploadTargetConfig::new(
                UploadMethod::Sftp,
                "ssh.upload.debian.org",
                "/srv/upload.debian.org/UploadQueue",
                None,
                None,
            ),
        ),
    ]
}

/// Resolve an upload target spec against the config's `[upload.targets]`
/// table merged field-by-field over a same-named builtin.
pub fn resolve_target(
    spec: &str,
    config_targets: Option<&HashMap<String, UploadTargetConfig>>,
) -> anyhow::Result<UploadTarget> {
    let (name, parameter) = parse_target_spec(spec);

    let builtin = builtin_targets()
        .into_iter()
        .find(|(builtin_name, _)| *builtin_name == name)
        .map(|(_, config)| config);

    let config = match (builtin, config_targets.and_then(|t| t.get(&name))) {
        (Some(mut builtin), Some(overlay)) => {
            merge_target_config(&mut builtin, overlay);
            builtin
        }
        (Some(builtin), None) => builtin,
        (None, Some(config)) => config.clone(),
        (None, None) => bail!(
            "unknown upload target '{name}'; configure [upload.targets.{name}] or use a builtin: ppa, ubuntu, debian"
        ),
    };

    Ok(UploadTarget {
        name,
        parameter,
        config,
    })
}

/// Field-by-field merge of `overlay` into `base`; unset overlay fields
/// keep the builtin's values.
fn merge_target_config(base: &mut UploadTargetConfig, overlay: &UploadTargetConfig) {
    if !overlay.server.is_empty() {
        base.server = overlay.server.clone();
    }
    if !overlay.incoming.is_empty() {
        base.incoming = overlay.incoming.clone();
    }
    if overlay.login.is_some() {
        base.login = overlay.login.clone();
    }
    if overlay.port.is_some() {
        base.port = overlay.port;
    }
    if !overlay.pre_upload_commands.is_empty() {
        base.pre_upload_commands = overlay.pre_upload_commands.clone();
    }
    if overlay.method.is_some() {
        base.method = overlay.method;
    }
    base.tofu_hostkey = overlay.tofu_hostkey;
}

/// The `[upload]` config section.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UploadConfig {
    /// Named upload targets, merged over the builtins.
    pub targets: HashMap<String, UploadTargetConfig>,
}

/// CLI overrides for one upload invocation, layered over the
/// resolved target (like `DriverOverrides` for builds).
#[derive(Debug, Default, Clone)]
pub struct UploadOverrides {
    pub method: Option<UploadMethod>,
    pub server: Option<String>,
    pub incoming: Option<String>,
    pub login: Option<String>,
    pub port: Option<u16>,
}

impl UploadTarget {
    pub fn apply_overrides(&mut self, overrides: &UploadOverrides) {
        if let Some(method) = overrides.method {
            self.config.method = Some(method);
        }
        if let Some(server) = &overrides.server {
            self.config.server = server.clone();
        }
        if let Some(incoming) = &overrides.incoming {
            self.config.incoming = incoming.clone();
        }
        if let Some(login) = &overrides.login {
            self.config.login = Some(login.clone());
        }
        if let Some(port) = overrides.port {
            self.config.port = Some(port);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_spec() {
        assert_eq!(
            parse_target_spec("ppa:sfttech/debmagic"),
            ("ppa".to_string(), Some("sfttech/debmagic".to_string()))
        );
        assert_eq!(parse_target_spec("ubuntu"), ("ubuntu".to_string(), None));
        assert_eq!(
            parse_target_spec("host:path:with:colons"),
            ("host".to_string(), Some("path:with:colons".to_string()))
        );
    }

    #[test]
    fn test_substitute_placeholders() {
        assert_eq!(
            substitute_placeholders("~{target}/ubuntu", &[("target", "sfttech/debmagic")]),
            "~sfttech/debmagic/ubuntu"
        );
        assert_eq!(
            substitute_placeholders("no placeholders", &[("target", "x")]),
            "no placeholders"
        );
    }

    #[test]
    fn test_resolve_builtin() {
        let target = resolve_target("debian", None).unwrap();
        assert_eq!(target.config.server, "ssh.upload.debian.org");
        assert_eq!(target.config.incoming, "/srv/upload.debian.org/UploadQueue");
        assert_eq!(target.config.method, Some(UploadMethod::Sftp));
    }

    #[test]
    fn test_resolve_unknown() {
        assert!(resolve_target("nonexistent", None).is_err());
    }

    #[test]
    fn test_resolve_merge_over_builtin() {
        let mut targets = HashMap::new();
        targets.insert(
            "ubuntu".to_string(),
            UploadTargetConfig::new(UploadMethod::Scp, "", "my-incoming", None, None),
        );
        let target = resolve_target("ubuntu", Some(&targets)).unwrap();
        // server comes from the builtin, incoming/method from the config
        assert_eq!(target.config.server, "upload.ubuntu.com");
        assert_eq!(target.config.incoming, "my-incoming");
        assert_eq!(target.config.method, Some(UploadMethod::Scp));
    }

    #[test]
    fn test_resolve_unset_method_not_overriding() {
        // an unset method field keeps the builtin's
        let mut targets = HashMap::new();
        targets.insert(
            "debian".to_string(),
            UploadTargetConfig {
                method: None,
                server: "other.host".to_string(),
                incoming: String::new(),
                login: None,
                port: None,
                tofu_hostkey: true,
                pre_upload_commands: Vec::new(),
            },
        );
        let target = resolve_target("debian", Some(&targets)).unwrap();
        assert_eq!(target.config.method, Some(UploadMethod::Sftp));
    }

    #[test]
    fn test_incoming_dir_parameter_substitution() {
        let target = resolve_target("ppa:sfttech/debmagic", None).unwrap();
        assert_eq!(target.incoming_dir(), "~sfttech/debmagic/ubuntu");
    }

    #[test]
    fn test_config_target_without_builtin() {
        let mut targets = HashMap::new();
        targets.insert(
            "myhost".to_string(),
            UploadTargetConfig::new(
                UploadMethod::Scp,
                "example.com",
                "/srv/incoming",
                Some("sfttech"),
                Some(2222),
            ),
        );
        let target = resolve_target("myhost", Some(&targets)).unwrap();
        assert_eq!(target.config.server, "example.com");
        assert_eq!(target.config.port, Some(2222));
    }
}
