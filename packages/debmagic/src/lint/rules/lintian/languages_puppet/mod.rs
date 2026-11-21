//! Rules for lintian's `languages/puppet` Check.

mod ln1113_puppet_module_but_not_dh_puppet;

pub(crate) use ln1113_puppet_module_but_not_dh_puppet::PuppetModuleButNotDhPuppet;
