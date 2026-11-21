//! Rules for lintian's `fields/multi-arch` Check.

mod ln1054_font_package_not_multi_arch_foreign;
mod ln1055_illegal_multi_arch_value;
mod ln1056_multi_arch_same_package_has_arch_specific_overrides;
mod ln1057_unknown_multi_arch_value;

pub(crate) use ln1054_font_package_not_multi_arch_foreign::FontPackageNotMultiArchForeign;
pub(crate) use ln1055_illegal_multi_arch_value::IllegalMultiArchValue;
pub(crate) use ln1056_multi_arch_same_package_has_arch_specific_overrides::MultiArchSamePackageHasArchSpecificOverrides;
pub(crate) use ln1057_unknown_multi_arch_value::UnknownMultiArchValue;
