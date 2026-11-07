//! Rules for lintian's `apache2` Check.
//!
//! `scan` recognises Apache module and web-application packages and reads
//! shell maintainer scripts. Each Rule owns one Tag.

mod ln0388_apache2_configuration_files_need_conf_suffix;
mod ln0390_apache2_reverse_dependency_calls_wrapper_script;
mod ln0391_apache2_unparsable_dependency;
mod scan;

pub(crate) use ln0388_apache2_configuration_files_need_conf_suffix::Apache2ConfigurationFilesNeedConfSuffix;
pub(crate) use ln0390_apache2_reverse_dependency_calls_wrapper_script::Apache2ReverseDependencyCallsWrapperScript;
pub(crate) use ln0391_apache2_unparsable_dependency::Apache2UnparsableDependency;
