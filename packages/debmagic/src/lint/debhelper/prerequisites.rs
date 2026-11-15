//! Which build dependency provides a dh command or a dh addon.
//!
//! The tables are lintian 2.139.0's `data/debhelper/commands.json` and
//! `data/debhelper/add_ons.json` (generated from the archive's Contents), and
//! the hand-maintained overrides in `lib/Lintian/Check/Debhelper.pm`
//! (`%DH_COMMAND_MANUAL_PREREQUISITES`, `%DH_ADDON_MANUAL_PREREQUISITES`),
//! which win over the generated entries. A prerequisite is a Package relation
//! condition in lintian's spelling: every installing package with `:any`,
//! joined by ` | `, with `debhelper-compat` appended when `debhelper` is
//! among them.

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

/// lintian 2.139.0 `data/debhelper/commands.json`.
static COMMANDS: LazyLock<HashMap<String, Installers>> = LazyLock::new(|| {
    serde_json::from_str::<CommandsFile>(include_str!("../data/debhelper/commands.json"))
        .expect("vendored debhelper/commands.json")
        .commands
});

/// lintian 2.139.0 `data/debhelper/add_ons.json`.
static ADD_ONS: LazyLock<HashMap<String, Installers>> = LazyLock::new(|| {
    serde_json::from_str::<AddOnsFile>(include_str!("../data/debhelper/add_ons.json"))
        .expect("vendored debhelper/add_ons.json")
        .add_ons
});

/// lintian 2.139.0 `%DH_COMMAND_MANUAL_PREREQUISITES`.
const COMMAND_MANUAL: &[(&str, &str)] = &[
    ("dh_apache2", "dh-apache2:any | apache2-dev:any"),
    (
        "dh_autoreconf_clean",
        "dh-autoreconf:any | debhelper:any (>= 9.20160403~) | debhelper-compat:any",
    ),
    (
        "dh_autoreconf",
        "dh-autoreconf:any | debhelper:any (>= 9.20160403~) | debhelper-compat:any",
    ),
    ("dh_dkms", "dh-dkms:any | dh-sequence-dkms:any"),
    (
        "dh_girepository",
        "gobject-introspection:any | dh-sequence-gir:any",
    ),
    ("dh_gnome", "gnome-pkg-tools:any | dh-sequence-gnome:any"),
    (
        "dh_gnome_clean",
        "gnome-pkg-tools:any | dh-sequence-gnome:any",
    ),
    ("dh_lv2config", "lv2core:any"),
    (
        "dh_make_pgxs",
        "postgresql-server-dev-all:any | postgresql-all:any",
    ),
    (
        "dh_nativejava",
        "gcj-native-helper:any | default-jdk-builddep:any",
    ),
    (
        "dh_pgxs_test",
        "postgresql-server-dev-all:any | postgresql-all:any",
    ),
    (
        "dh_python3",
        "dh-python:any | dh-sequence-python3:any | pybuild-plugin-pyproject:any",
    ),
    (
        "dh_sphinxdoc",
        "sphinx:any | python-sphinx:any | python3-sphinx:any | dh-sequence-sphinxdoc:any",
    ),
    ("dh_xine", "libxine-dev:any | libxine2-dev:any"),
];

/// lintian 2.139.0 `%DH_ADDON_MANUAL_PREREQUISITES`.
const ADD_ON_MANUAL: &[(&str, &str)] = &[
    (
        "ada_library",
        "dh-ada-library:any | dh-sequence-ada-library:any",
    ),
    ("apache2", "dh-apache2:any | apache2-dev:any"),
    (
        "autoreconf",
        "dh-autoreconf:any | debhelper:any (>= 9.20160403~) | debhelper-compat:any",
    ),
    ("cli", "cli-common-dev:any | dh-sequence-cli:any"),
    (
        "dwz",
        "debhelper:any | debhelper-compat:any | dh-sequence-dwz:any",
    ),
    (
        "installinitramfs",
        "debhelper:any | debhelper-compat:any | dh-sequence-installinitramfs:any",
    ),
    ("gir", "gobject-introspection:any | dh-sequence-gir:any"),
    ("gnome", "gnome-pkg-tools:any | dh-sequence-gnome:any"),
    ("lv2config", "lv2core:any"),
    ("nodejs", "pkg-js-tools:any | dh-sequence-nodejs:any"),
    ("perl_dbi", "libdbi-perl:any | dh-sequence-perl-dbi:any"),
    (
        "perl_imager",
        "libimager-perl:any | dh-sequence-perl-imager:any",
    ),
    ("pgxs", "postgresql-server-dev-all:any | postgresql-all:any"),
    (
        "pgxs_loop",
        "postgresql-server-dev-all:any | postgresql-all:any",
    ),
    ("pypy", "dh-python:any | dh-sequence-pypy:any"),
    (
        "python3",
        "python3:any | python3-all:any | python3-dev:any | python3-all-dev:any | dh-sequence-python3:any",
    ),
    (
        "scour",
        "scour:any | python-scour:any | dh-sequence-scour:any",
    ),
    (
        "sphinxdoc",
        "sphinx:any | python-sphinx:any | python3-sphinx:any | dh-sequence-sphinxdoc:any",
    ),
    (
        "systemd",
        "debhelper:any (>= 9.20160709~) | debhelper-compat:any | dh-sequence-systemd:any | dh-systemd:any",
    ),
    ("vim_addon", "dh-vim-addon:any | dh-sequence-vim-addon:any"),
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
// Read by the follow-on `missing-build-dependency-for-dh-addon`.
#[allow(dead_code)]
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
            .map(|package| format!("{package}:any"))
            .collect::<Vec<_>>()
            .join(" | "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_commands_broaden_to_any() {
        assert_eq!(
            command_prerequisite("dh_lisp").as_deref(),
            Some("dh-lisp:any")
        );
        assert_eq!(
            command_prerequisite("dh_makeclilibs").as_deref(),
            Some("cli-common-dev:any | mono-devel:any")
        );
    }

    #[test]
    fn debhelper_commands_add_debhelper_compat() {
        assert_eq!(
            command_prerequisite("dh_md5sums").as_deref(),
            Some("debhelper:any | debhelper-compat:any")
        );
    }

    #[test]
    fn manual_commands_win_over_generated_ones() {
        assert_eq!(
            command_prerequisite("dh_python3").as_deref(),
            Some("dh-python:any | dh-sequence-python3:any | pybuild-plugin-pyproject:any")
        );
        assert_eq!(
            command_prerequisite("dh_apache2").as_deref(),
            Some("dh-apache2:any | apache2-dev:any")
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
            Some("pkg-php-tools:any | dh-sequence-phpcomposer:any")
        );
        // `debhelper` is listed, so `debhelper-compat` is appended again.
        assert_eq!(
            addon_prerequisite("single_binary").as_deref(),
            Some(
                "dh-sequence-single-binary:any | debhelper-compat:any | debhelper:any | debhelper-compat:any"
            )
        );
        assert_eq!(
            addon_prerequisite("systemd").as_deref(),
            Some(
                "debhelper:any (>= 9.20160709~) | debhelper-compat:any | dh-sequence-systemd:any | dh-systemd:any"
            )
        );
        assert_eq!(addon_prerequisite("will-never-exist"), None);
    }
}
