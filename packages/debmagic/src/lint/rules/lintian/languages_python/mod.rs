//! Rules for lintian's `languages/python` Check.
//!
//! `python_version` reads the `X-Python-Version` and `X-Python3-Version`
//! fields. The path Rules match installed names on their own.

mod ln0093_ancient_python_version_field;
mod ln0103_malformed_python_version;
mod ln0104_python_version_current_is_deprecated;
mod ln0105_source_package_encodes_python_version;
mod ln0129_package_installs_deprecated_python2_path;
mod ln0130_file_in_usr_lib_site_python;
mod ln0131_third_party_package_in_python_dir;
mod python_version;

pub(crate) use ln0093_ancient_python_version_field::AncientPythonVersionField;
pub(crate) use ln0103_malformed_python_version::MalformedPythonVersion;
pub(crate) use ln0104_python_version_current_is_deprecated::PythonVersionCurrentIsDeprecated;
pub(crate) use ln0105_source_package_encodes_python_version::SourcePackageEncodesPythonVersion;
pub(crate) use ln0129_package_installs_deprecated_python2_path::PackageInstallsDeprecatedPython2Path;
pub(crate) use ln0130_file_in_usr_lib_site_python::FileInUsrLibSitePython;
pub(crate) use ln0131_third_party_package_in_python_dir::ThirdPartyPackageInPythonDir;
