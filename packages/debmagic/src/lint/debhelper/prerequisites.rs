//! Which build dependency provides a dh command or a dh addon.
//!
//! The tables are lintian 2.144.0's `data/debhelper/commands.json` and
//! `data/debhelper/add_ons.json` (generated from the archive's Contents), and
//! the hand-maintained overrides in `lib/Lintian/Check/Debhelper.pm`
//! (`%DH_COMMAND_MANUAL_PREREQUISITES`, `%DH_ADDON_MANUAL_PREREQUISITES`),
//! which win over the generated entries. A prerequisite is a Package relation
//! condition in lintian's spelling: every installing package, joined by
//! ` | `, with `debhelper-compat` appended when `debhelper` is among them.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct Installers {
    installed_by: Vec<String>,
}

#[derive(Deserialize)]
struct CommandsFile {
    commands: HashMap<String, Installers>,
}

#[derive(Deserialize)]
struct AddOnsFile {
    add_ons: HashMap<String, Installers>,
}

/// lintian 2.144.0 `data/debhelper/commands.json`.
static COMMANDS: LazyLock<HashMap<String, Installers>> = LazyLock::new(|| {
    serde_json::from_str::<CommandsFile>(include_str!("../data/debhelper/commands.json"))
        .expect("vendored debhelper/commands.json")
        .commands
});

/// lintian 2.144.0 `data/debhelper/add_ons.json`.
static ADD_ONS: LazyLock<HashMap<String, Installers>> = LazyLock::new(|| {
    serde_json::from_str::<AddOnsFile>(include_str!("../data/debhelper/add_ons.json"))
        .expect("vendored debhelper/add_ons.json")
        .add_ons
});

/// lintian 2.144.0 `%DH_COMMAND_MANUAL_PREREQUISITES`.
const COMMAND_MANUAL: &[(&str, &str)] = &[
    ("dh_apache2", "dh-apache2 | apache2-dev"),
    (
        "dh_autoreconf_clean",
        "dh-autoreconf | debhelper (>= 9.20160403~) | debhelper-compat",
    ),
    (
        "dh_autoreconf",
        "dh-autoreconf | debhelper (>= 9.20160403~) | debhelper-compat",
    ),
    ("dh_dkms", "dh-dkms | dh-sequence-dkms"),
    ("dh_girepository", "gobject-introspection | dh-sequence-gir"),
    ("dh_gnome", "gnome-pkg-tools | dh-sequence-gnome"),
    ("dh_gnome_clean", "gnome-pkg-tools | dh-sequence-gnome"),
    ("dh_make_pgxs", "postgresql-server-dev-all | postgresql-all"),
    ("dh_pgxs_test", "postgresql-server-dev-all | postgresql-all"),
    (
        "dh_python3",
        "dh-python | dh-sequence-python3 | pybuild-plugin-pyproject",
    ),
    (
        "dh_sphinxdoc",
        "sphinx | python3-sphinx | dh-sequence-sphinxdoc",
    ),
    ("dh_xine", "libxine2-dev"),
];

/// lintian 2.144.0 `%DH_ADDON_MANUAL_PREREQUISITES`.
const ADD_ON_MANUAL: &[(&str, &str)] = &[
    ("ada_library", "dh-ada-library | dh-sequence-ada-library"),
    ("apache2", "dh-apache2 | apache2-dev"),
    (
        "autoreconf",
        "dh-autoreconf | debhelper (>= 9.20160403~) | debhelper-compat",
    ),
    ("cli", "cli-common-dev | dh-sequence-cli"),
    ("dwz", "debhelper | debhelper-compat | dh-sequence-dwz"),
    (
        "installinitramfs",
        "debhelper | debhelper-compat | dh-sequence-installinitramfs",
    ),
    ("gir", "gobject-introspection | dh-sequence-gir"),
    ("gnome", "gnome-pkg-tools | dh-sequence-gnome"),
    ("nodejs", "pkg-js-tools | dh-sequence-nodejs"),
    ("perl_dbi", "libdbi-perl | dh-sequence-perl-dbi"),
    ("perl_imager", "libimager-perl | dh-sequence-perl-imager"),
    ("pgxs", "postgresql-server-dev-all | postgresql-all"),
    ("pgxs_loop", "postgresql-server-dev-all | postgresql-all"),
    ("pypy", "dh-python | dh-sequence-pypy"),
    (
        "python3",
        "python3:any | python3-all:any | python3-dev:any | python3-all-dev:any | dh-sequence-python3",
    ),
    ("scour", "scour | dh-sequence-scour"),
    (
        "sphinxdoc",
        "sphinx | python3-sphinx | dh-sequence-sphinxdoc",
    ),
    (
        "systemd",
        "debhelper (>= 9.20160709~) | debhelper-compat | dh-sequence-systemd | dh-systemd",
    ),
    ("vim_addon", "dh-vim-addon | dh-sequence-vim-addon"),
];

/// The build prerequisite lintian records for a `dh_*` command seen in
/// `debian/rules`. `None` when neither the overrides nor `commands.json`
/// know the command.
pub(crate) fn command_prerequisite(command: &str) -> Option<String> {
    lookup(COMMAND_MANUAL, &COMMANDS, command)
}

/// The build prerequisite lintian records for a dh addon named by `--with`,
/// after lintian turns `-` into `_`. `None` when neither the overrides nor
/// `add_ons.json` know the addon (lintian then records an empty condition,
/// which every relation satisfies).
pub(crate) fn addon_prerequisite(addon: &str) -> Option<String> {
    lookup(ADD_ON_MANUAL, &ADD_ONS, addon)
}

fn lookup(
    manual: &[(&str, &str)],
    generated: &HashMap<String, Installers>,
    name: &str,
) -> Option<String> {
    if let Some((_, prerequisite)) = manual.iter().find(|(key, _)| *key == name) {
        return Some((*prerequisite).to_string());
    }
    let installers = &generated.get(name)?.installed_by;
    let mut packages: Vec<&str> = installers.iter().map(String::as_str).collect();
    if packages.contains(&"debhelper") {
        packages.push("debhelper-compat");
    }
    if packages.is_empty() {
        return None;
    }
    Some(
        packages
            .iter()
            .map(|package| (*package).to_string())
            .collect::<Vec<_>>()
            .join(" | "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_commands_join_installers() {
        assert_eq!(command_prerequisite("dh_lisp").as_deref(), Some("dh-lisp"));
        assert_eq!(
            command_prerequisite("dh_makeclilibs").as_deref(),
            Some("cli-common-dev | mono-devel")
        );
    }

    #[test]
    fn debhelper_commands_add_debhelper_compat() {
        assert_eq!(
            command_prerequisite("dh_md5sums").as_deref(),
            Some("debhelper | debhelper-compat")
        );
    }

    #[test]
    fn manual_commands_win_over_generated_ones() {
        assert_eq!(
            command_prerequisite("dh_python3").as_deref(),
            Some("dh-python | dh-sequence-python3 | pybuild-plugin-pyproject")
        );
        assert_eq!(
            command_prerequisite("dh_apache2").as_deref(),
            Some("dh-apache2 | apache2-dev")
        );
    }

    #[test]
    fn unknown_commands_have_no_prerequisite() {
        assert_eq!(command_prerequisite("dh_will_never_exist"), None);
        assert_eq!(command_prerequisite("dh"), None);
    }

    #[test]
    fn addons_use_their_own_tables() {
        assert_eq!(
            addon_prerequisite("phpcomposer").as_deref(),
            Some("pkg-php-tools | dh-sequence-phpcomposer")
        );
        // `debhelper` is listed, so `debhelper-compat` is appended again.
        assert_eq!(
            addon_prerequisite("single_binary").as_deref(),
            Some("dh-sequence-single-binary | debhelper-compat | debhelper | debhelper-compat")
        );
        assert_eq!(
            addon_prerequisite("systemd").as_deref(),
            Some(
                "debhelper (>= 9.20160709~) | debhelper-compat | dh-sequence-systemd | dh-systemd"
            )
        );
        assert_eq!(addon_prerequisite("will-never-exist"), None);
    }
}
