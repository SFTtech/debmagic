//! Rules for lintian's `fields/architecture` Check.
//!
//! Each Rule owns one Tag. They read the first Architecture value of the
//! Binary package, as lintian's `installable` does.

use crate::lint::context::BinaryPackageContext;

mod ln0771_aspell_package_not_arch_all;
mod ln0772_documentation_package_not_architecture_independent;
mod ln0900_arch_wildcard_in_binary_package;
mod ln0901_magic_arch_in_arch_list;

pub(crate) use ln0771_aspell_package_not_arch_all::AspellPackageNotArchAll;
pub(crate) use ln0772_documentation_package_not_architecture_independent::DocumentationPackageNotArchitectureIndependent;
pub(crate) use ln0900_arch_wildcard_in_binary_package::ArchWildcardInBinaryPackage;
pub(crate) use ln0901_magic_arch_in_arch_list::MagicArchInArchList;

/// First Architecture value of the Binary package, `None` when not declared.
pub(super) fn first_architecture(ctx: &BinaryPackageContext<'_>) -> Option<String> {
    ctx.installation_control()
        .fields()
        .get("Architecture")?
        .split_whitespace()
        .next()
        .map(str::to_string)
}
