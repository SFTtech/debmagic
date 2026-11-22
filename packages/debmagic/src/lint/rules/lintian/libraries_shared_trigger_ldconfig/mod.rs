//! Rules for lintian's `libraries/shared/trigger/ldconfig` Check.

mod ln1414_lacks_ldconfig_trigger;
mod ln1415_package_has_unnecessary_activation_of_ldconfig_trigger;

pub(crate) use ln1414_lacks_ldconfig_trigger::LacksLdconfigTrigger;
pub(crate) use ln1415_package_has_unnecessary_activation_of_ldconfig_trigger::PackageHasUnnecessaryActivationOfLdconfigTrigger;
