//! Rules for lintian's `fields/version` Check.
//!
//! The Version is read from the Dsc when there is one, otherwise the newest
//! `debian/changelog` heading on a Source tree, and from Installation control
//! on a Binary package. Each Rule emits one Tag.

use crate::lint::context::SourceContext;

mod ln0373_debian_revision_is_zero;
mod ln0374_debian_revision_not_well_formed;

pub(crate) use ln0373_debian_revision_is_zero::DebianRevisionIsZero;
pub(crate) use ln0374_debian_revision_not_well_formed::DebianRevisionNotWellFormed;

/// The Version of a Source package (its Dsc) or of a Source tree (the newest
/// changelog heading). `None` when neither gives one.
fn source_version(ctx: &SourceContext<'_>) -> Option<String> {
    if let Some(dsc) = ctx.dsc() {
        return dsc.fields().unfolded_value("Version");
    }
    let entry = ctx.debian_changelog()?.entries().first()?;
    Some(entry.version().to_string())
}
