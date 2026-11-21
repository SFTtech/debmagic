//! Rules for lintian's `apt` Check.

mod ln1001_package_installs_apt_keyring;
mod ln1002_package_installs_apt_preferences;
mod ln1003_package_installs_apt_sources;

pub(crate) use ln1001_package_installs_apt_keyring::PackageInstallsAptKeyring;
pub(crate) use ln1002_package_installs_apt_preferences::PackageInstallsAptPreferences;
pub(crate) use ln1003_package_installs_apt_sources::PackageInstallsAptSources;
