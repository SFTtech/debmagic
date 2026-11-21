//! Rules for lintian's `files/ld-so` Check.

mod ln1082_package_modifies_ld_so_search_path;

pub(crate) use ln1082_package_modifies_ld_so_search_path::PackageModifiesLdSoSearchPath;
