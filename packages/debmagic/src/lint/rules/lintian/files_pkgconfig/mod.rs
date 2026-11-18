//! Rules for lintian's `files/pkgconfig` Check.

mod ln0727_pkg_config_bad_directive;

pub(crate) use ln0727_pkg_config_bad_directive::PkgConfigBadDirective;
mod ln0730_pkg_config_unavailable_for_cross_compilation;

pub(crate) use ln0730_pkg_config_unavailable_for_cross_compilation::PkgConfigUnavailableForCrossCompilation;
mod ln0732_pkg_config_multi_arch_wrong_dir;

pub(crate) use ln0732_pkg_config_multi_arch_wrong_dir::PkgConfigMultiArchWrongDir;
