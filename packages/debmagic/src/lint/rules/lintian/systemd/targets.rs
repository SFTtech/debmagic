//! Target names hardcoded in `Lintian::Check::Systemd`.
//!
//! The obsolete `After=` names and the usual `WantedBy=` names are not loaded
//! from a data file. Alias, `BindTo=`, and `User=`/`Group=` do not consult
//! them; they live here so the check's tables are not read from lintian at
//! test time.

/// Usual `WantedBy=` targets.
#[allow(dead_code)]
pub(super) const USUAL_TARGETS: &[&str] = &[
    "default.target",
    "graphical.target",
    "multi-user.target",
    "network-online.target",
    "sleep.target",
    "sysinit.target",
];

/// Obsolete `After=` targets (`syslog.target` and `dbus.target`).
#[allow(dead_code)]
pub(super) const OBSOLETE_TARGETS: &[&str] = &["syslog.target", "dbus.target"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_the_systemd_check() {
        assert!(USUAL_TARGETS.contains(&"multi-user.target"));
        assert_eq!(USUAL_TARGETS.len(), 6);
        assert_eq!(OBSOLETE_TARGETS, ["syslog.target", "dbus.target"]);
    }
}
