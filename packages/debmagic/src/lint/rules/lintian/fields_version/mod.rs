//! Rules for lintian's `fields/version` Check.
//!
//! The Version is read from the Dsc when there is one, otherwise the newest
//! `debian/changelog` heading on a Source tree, and from Installation control
//! on a Binary package. Each Rule emits one Tag.

use crate::lint::context::SourceContext;
use crate::lint::source_format;
use crate::lint::source_package::strip_epoch;

mod ln0373_debian_revision_is_zero;
mod ln0374_debian_revision_not_well_formed;
mod ln0755_binary_nmu_debian_revision_in_source;
mod ln0805_dfsg_version_misspelled;
mod ln0806_dfsg_version_in_native_package;
mod ln0925_bad_version_number;
mod ln1554_anticipated_repack_count;
mod ln1555_dot_before_repack_count;
mod ln1556_dfsg_version_with_period;
mod ln1558_dfsg_version_with_tilde;
mod ln1797_invalid_version_number_for_derivative;

pub(crate) use ln0373_debian_revision_is_zero::DebianRevisionIsZero;
pub(crate) use ln0374_debian_revision_not_well_formed::DebianRevisionNotWellFormed;
pub(crate) use ln0755_binary_nmu_debian_revision_in_source::BinaryNmuDebianRevisionInSource;
pub(crate) use ln0805_dfsg_version_misspelled::DfsgVersionMisspelled;
pub(crate) use ln0806_dfsg_version_in_native_package::DfsgVersionInNativePackage;
pub(crate) use ln0925_bad_version_number::BadVersionNumber;
pub(crate) use ln1554_anticipated_repack_count::AnticipatedRepackCount;
pub(crate) use ln1555_dot_before_repack_count::DotBeforeRepackCount;
pub(crate) use ln1556_dfsg_version_with_period::DfsgVersionWithPeriod;
pub(crate) use ln1558_dfsg_version_with_tilde::DfsgVersionWithTilde;
pub(crate) use ln1797_invalid_version_number_for_derivative::InvalidVersionNumberForDerivative;

/// The Version of a Source package (its Dsc) or of a Source tree (the newest
/// changelog heading). `None` when neither gives one.
pub(super) fn source_version(ctx: &SourceContext<'_>) -> Option<String> {
    if let Some(dsc) = ctx.dsc() {
        return dsc.fields().unfolded_value("Version");
    }
    let entry = ctx.debian_changelog()?.entries().first()?;
    Some(entry.version().to_string())
}

/// Lintian's `native`. The format decides first ([`source_format::nativeness`]).
/// Otherwise the Version without its epoch is
/// native when it has no `.diff.gz`: a Dsc lists it in Files, and a Source
/// tree has none, so its Version must have no revision.
pub(super) fn is_native(ctx: &mut SourceContext<'_>) -> bool {
    if let Some(native) = source_format::nativeness(&source_format::lintian_format(ctx)) {
        return native;
    }
    let Some(version) = source_version(ctx) else {
        return false;
    };
    let version = strip_epoch(&version);
    if version.is_empty() {
        return false;
    }
    match ctx.dsc() {
        Some(dsc) => {
            let source = dsc.fields().get("Source").unwrap_or_default().trim();
            let diff = format!("{source}_{version}.diff.gz");
            !dsc.files().contains(&diff)
        }
        None => !version.contains('-'),
    }
}
