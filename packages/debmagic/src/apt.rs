//! Every apt call debmagic makes in an Environment: the one-time setup of a
//! container, refreshing its packages, and installing what a run needs in
//! one transaction.

use std::{fs, io, os::unix::fs::PermissionsExt, path::Path};

use anyhow::Context;
use debmagic_common::{
    apt::{self, Archives, PackageSettings, Packages},
    debian::control::BuildRelations,
    distro::DistroVersion,
};
use serde::{Deserialize, Serialize};

use crate::driver::EnvironmentDriver;
use crate::time::RefreshPolicy;

/// The `[apt]` section: where an Environment's packages come from. Not
/// used by the bare driver, which builds on the host's own sources.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct AptConfig {
    /// Mirror replacing the distro's default archive.
    pub mirror: Option<String>,
    /// When a reused Environment runs apt-get update/upgrade again.
    pub update_age: RefreshPolicy,
    /// All newer packages from the suite the release's updates migrate
    /// from.
    pub proposed: bool,
    /// What nothing else provides from Debian experimental, for unstable
    /// and testing builds.
    pub experimental: bool,
    /// Where to take packages from, or which version, by apt preferences
    /// pattern.
    pub packages: Packages,
}

impl AptConfig {
    /// The archives beyond its release an Environment for `distro` takes
    /// packages from.
    pub fn archives(&self, distro: &DistroVersion) -> anyhow::Result<Archives> {
        let packages = self.packages.clone();
        Archives::for_distro(distro, self.proposed, self.experimental, packages)
            .context("invalid apt package settings")
    }
}

/// The `[apt]` settings given on the command line.
#[derive(Debug, Clone, Default)]
pub struct AptOverrides {
    pub mirror: Option<String>,
    pub update_age: Option<RefreshPolicy>,
    pub proposed: Option<bool>,
    pub experimental: Option<bool>,
    /// Replacing the configured settings of the same pattern.
    pub packages: Vec<(String, PackageSettings)>,
}

impl AptOverrides {
    pub fn apply(self, config: &mut AptConfig) {
        if let Some(mirror) = self.mirror {
            config.mirror = Some(mirror);
        }
        if let Some(update_age) = self.update_age {
            config.update_age = update_age;
        }
        if let Some(proposed) = self.proposed {
            config.proposed = proposed;
        }
        if let Some(experimental) = self.experimental {
            config.experimental = experimental;
        }
        config.packages.extend(self.packages);
    }
}

const ENV: [(&str, &str); 1] = [("DEBIAN_FRONTEND", "noninteractive")];

/// Installed with every [`Install`], so a container needs no apt run at
/// creation.
const BASE_PACKAGES: &[&str] = &["dpkg-dev"];

/// Name of the setup tree in an Environment's temp dir.
pub const SETUP_DIR: &str = "apt-setup";

/// Improve performance in build containers
/// no recommends, no translations or index diffs to fetch,
/// no fsync, no man-db rebuilds, keep changed conffiles.
const SETUP_FILES: &[(&str, &str, u32)] = &[
    (
        "etc/apt/apt.conf.d/90debmagic",
        "APT::Install-Recommends \"false\";\n\
         Acquire::Languages \"none\";\n\
         Acquire::PDiffs \"false\";\n\
         Dpkg::Options { \"--force-confdef\"; \"--force-confold\"; };\n",
        0o644,
    ),
    ("etc/dpkg/dpkg.cfg.d/debmagic", "force-unsafe-io\n", 0o644),
];

/// Keeps maintainer scripts from starting services, which builds never use.
const POLICY_RC_D: (&str, &str, u32) = ("usr/sbin/policy-rc.d", "#!/bin/sh\nexit 101\n", 0o755);

const SOURCES_FILE: &str = "etc/apt/sources.list.d/debmagic.sources";
const PREFERENCES_FILE: &str = "etc/apt/preferences.d/debmagic.pref";

const MAN_DB_PRESEED: &str = "'man-db man-db/auto-update boolean false'";

/// The base images' own sources, replaced by [`SOURCES_FILE`].
const DEFAULT_SOURCES: &[&str] = &[
    "/etc/apt/sources.list",
    "/etc/apt/sources.list.d/ubuntu.sources",
    "/etc/apt/sources.list.d/debian.sources",
];

/// The apt and dpkg configuration of a fresh container: a file tree to copy
/// over its root, written on the host so the container needs nothing to
/// create it. Apt sources replace the base image's only for a `mirror` or
/// extra suites.
pub struct Setup<'a> {
    pub distro: &'a DistroVersion,
    pub mirror: Option<&'a str>,
    pub archives: &'a Archives,
    /// Let installed packages start their services, which tests may expect.
    pub services: bool,
}

impl Setup<'_> {
    fn replaces_sources(&self) -> bool {
        self.mirror.is_some() || self.archives.adds_suites()
    }

    /// The setup files apart from the apt sources.
    fn files(&self) -> impl Iterator<Item = (&'static str, &'static str, u32)> {
        SETUP_FILES
            .iter()
            .copied()
            .chain((!self.services).then_some(POLICY_RC_D))
    }

    /// Changes whenever the container would be set up differently, for
    /// Driver fingerprints.
    pub fn fingerprint(&self) -> String {
        self.files()
            .map(|(path, content, mode)| format!("{path}:{mode:o}:{content}"))
            .chain([
                self.script(""),
                self.mirror.unwrap_or_default().to_string(),
                format!("{:?}", self.archives),
            ])
            .collect::<Vec<_>>()
            .join("\0")
    }

    /// Write the setup tree into `dir`, replacing an old one.
    pub fn write(&self, dir: &Path) -> anyhow::Result<()> {
        let sources = self
            .replaces_sources()
            .then(|| apt::sources_file(self.distro, self.mirror, self.archives, host_dpkg_arch()))
            .transpose()
            .context("generating the apt sources failed")?;
        if dir.exists() {
            fs::remove_dir_all(dir)
                .with_context(|| format!("removing the old {} failed", dir.display()))?;
        }
        let preferences = apt::preferences_file(self.distro, self.archives);
        let mut files: Vec<(&str, &str, u32)> = self.files().collect();
        for (path, content) in [(SOURCES_FILE, &sources), (PREFERENCES_FILE, &preferences)] {
            files.extend(content.as_deref().map(|content| (path, content, 0o644)));
        }
        for (path, content, mode) in files {
            let path = dir.join(path);
            fs::create_dir_all(path.parent().expect("setup files are in a dir"))
                .and_then(|()| fs::write(&path, content))
                .and_then(|()| fs::set_permissions(&path, fs::Permissions::from_mode(mode)))
                .with_context(|| format!("writing {} failed", path.display()))?;
        }
        Ok(())
    }

    /// A `sh -c` command line applying the setup tree found at
    /// `dir_in_container`.
    pub fn script(&self, dir_in_container: &str) -> String {
        let mut steps = Vec::new();
        if self.replaces_sources() {
            steps.push(format!("rm -f {}", DEFAULT_SOURCES.join(" ")));
        }
        steps.push(format!("cp -R {dir_in_container}/. /"));
        // man-db's postinst recreates the flag file from debconf
        steps.push(format!("echo {MAN_DB_PRESEED} | debconf-set-selections"));
        steps.push("rm -f /var/lib/man-db/auto-update".to_string());
        steps.join(" && ")
    }
}

/// The dpkg architecture of containers, which run on the host's.
fn host_dpkg_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "i386",
        "aarch64" => "arm64",
        "powerpc64" => "ppc64el",
        "loongarch64" => "loong64",
        other => other,
    }
}

/// `apt-get update` and `upgrade`, so a cached image or a long-lived
/// Environment builds against current packages, like sbuild does.
pub fn refresh_commands() -> Vec<Vec<String>> {
    [&["update"][..], &["-y", "upgrade"]]
        .iter()
        .map(|args| apt_get(args.iter().copied()))
        .collect()
}

pub fn refresh(driver: &dyn EnvironmentDriver, cwd: &Path) -> io::Result<()> {
    run(driver, cwd, &refresh_commands())
}

/// What a run needs installed, in one apt transaction where the release's
/// apt allows it. [`BASE_PACKAGES`] are always part of it.
#[derive(Debug, Default)]
pub struct Install<'a> {
    pub packages: Vec<&'a str>,
    /// The build relations of the source tree in the install's working
    /// dir. Before `apt-get satisfy`, `apt-get build-dep .` installs all of
    /// them from its `debian/control` instead.
    pub build_relations: Option<BuildRelations>,
    /// Build profiles the build relations are evaluated with.
    pub profiles: Vec<String>,
}

/// The commands for `install`, preferring the packages of `target_release`
/// if given.
pub fn install_commands(
    satisfy: bool,
    install: &Install,
    target_release: Option<&str>,
) -> Vec<Vec<String>> {
    let mut options = vec!["-y", "-q", "--no-install-recommends"];
    if let Some(release) = target_release {
        options.extend(["-t", release]);
    }
    let mut profiled = apt_get(options.iter().copied());
    if !install.profiles.is_empty() {
        profiled.extend(["-P".to_string(), install.profiles.join(",")]);
    }
    let packages = BASE_PACKAGES.iter().chain(&install.packages).copied();

    if satisfy {
        let mut command = profiled;
        command.push("satisfy".to_string());
        command.extend(packages.map(str::to_string));
        if let Some(relations) = &install.build_relations {
            command.extend(relations.depends.iter().cloned());
            let conflicts = relations.conflicts.iter();
            command.extend(conflicts.map(|conflicts| format!("Conflicts: {conflicts}")));
        }
        return vec![command];
    }

    let mut commands = vec![apt_get(
        options.into_iter().chain(["install"]).chain(packages),
    )];
    if install.build_relations.is_some() {
        profiled.extend(["build-dep".to_string(), ".".to_string()]);
        commands.push(profiled);
    }
    commands
}

/// Install `install`, with all of a NotAutomatic suite like experimental
/// in `archives` retried preferring that suite: apt doesn't take a version
/// only that suite has otherwise, while preferring it upfront would take
/// everything it has.
pub fn install(
    driver: &dyn EnvironmentDriver,
    cwd: &Path,
    distro: &DistroVersion,
    archives: &Archives,
    install: &Install,
) -> io::Result<()> {
    let satisfy = apt::has_satisfy(distro);
    let result = run(driver, cwd, &install_commands(satisfy, install, None));
    match apt::retry_suite(archives) {
        Some(suite) if result.is_err() => {
            println!("debmagic: retrying preferring the packages from {suite}");
            run(
                driver,
                cwd,
                &install_commands(satisfy, install, Some(suite)),
            )
        }
        _ => result,
    }
}

/// `commands` as one `sh -c` command line.
pub fn shell_line(commands: &[Vec<String>]) -> String {
    commands
        .iter()
        .map(|command| {
            command
                .iter()
                .map(|arg| shell_quote(arg))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

/// `s` as a single POSIX shell word, quoted only if it needs to be.
pub fn shell_quote(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./=:,+@%".contains(&b));
    if plain {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn apt_get<S: Into<String>>(args: impl IntoIterator<Item = S>) -> Vec<String> {
    std::iter::once("apt-get".to_string())
        .chain(args.into_iter().map(Into::into))
        .collect()
}

fn run(driver: &dyn EnvironmentDriver, cwd: &Path, commands: &[Vec<String>]) -> io::Result<()> {
    for command in commands {
        let command: Vec<&str> = command.iter().map(String::as_str).collect();
        driver.run_command_checked(&command, cwd, true, &ENV)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_install() -> Install<'static> {
        Install {
            packages: vec!["build-essential"],
            build_relations: Some(BuildRelations {
                depends: vec!["debhelper-compat (= 13), libfoo-dev <!nocheck>".to_string()],
                conflicts: vec!["libbar-dev".to_string()],
            }),
            profiles: vec!["nocheck".to_string(), "nodoc".to_string()],
        }
    }

    #[test]
    fn satisfy_installs_everything_in_one_transaction() {
        assert_eq!(
            shell_line(&install_commands(true, &build_install(), None)),
            "apt-get -y -q --no-install-recommends -P nocheck,nodoc satisfy dpkg-dev build-essential \
             'debhelper-compat (= 13), libfoo-dev <!nocheck>' 'Conflicts: libbar-dev'"
        );
    }

    #[test]
    fn target_release_applies_to_every_command() {
        assert_eq!(
            shell_line(&install_commands(
                false,
                &build_install(),
                Some("experimental")
            )),
            "apt-get -y -q --no-install-recommends -t experimental install dpkg-dev build-essential && \
             apt-get -y -q --no-install-recommends -t experimental -P nocheck,nodoc build-dep ."
        );
    }

    #[test]
    fn without_satisfy_build_dep_reads_the_source_tree() {
        assert_eq!(
            shell_line(&install_commands(false, &build_install(), None)),
            "apt-get -y -q --no-install-recommends install dpkg-dev build-essential && \
             apt-get -y -q --no-install-recommends -P nocheck,nodoc build-dep ."
        );
    }

    #[test]
    fn plain_install_has_no_build_dep() {
        let install = Install {
            packages: vec!["autopkgtest"],
            ..Install::default()
        };
        assert_eq!(
            shell_line(&install_commands(false, &install, None)),
            "apt-get -y -q --no-install-recommends install dpkg-dev autopkgtest"
        );
    }

    #[test]
    fn setup_replaces_the_default_sources_only_with_its_own() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-apt-setup-{}", std::process::id()));
        let trixie = DistroVersion::new(debmagic_common::distro::Distro::Debian, "trixie", "13");

        let mut setup = Setup {
            distro: &trixie,
            mirror: None,
            archives: &Archives::default(),
            services: false,
        };
        setup.write(&dir)?;
        assert!(!dir.join(SOURCES_FILE).exists());
        assert!(!setup.script("/s").contains("sources.list"));
        let policy = fs::metadata(dir.join("usr/sbin/policy-rc.d"))?;
        assert_eq!(policy.permissions().mode() & 0o777, 0o755);
        let plain = setup.fingerprint();

        setup.services = true;
        setup.write(&dir)?;
        assert!(!dir.join("usr/sbin/policy-rc.d").exists());
        assert_ne!(setup.fingerprint(), plain);
        setup.services = false;

        setup.mirror = Some("http://m/debian");
        setup.write(&dir)?;
        assert_ne!(setup.fingerprint(), plain);
        assert!(fs::read_to_string(dir.join(SOURCES_FILE))?.contains("URIs: http://m/debian\n"));
        assert_eq!(
            setup.script("/s"),
            "rm -f /etc/apt/sources.list /etc/apt/sources.list.d/ubuntu.sources \
             /etc/apt/sources.list.d/debian.sources && cp -R /s/. / && \
             echo 'man-db man-db/auto-update boolean false' | debconf-set-selections && \
             rm -f /var/lib/man-db/auto-update"
        );
        fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
