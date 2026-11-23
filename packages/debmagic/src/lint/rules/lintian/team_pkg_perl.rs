//! Helpers shared by lintian's `team/pkg-perl/*` Checks.
//!
//! Each Check reads the Source package's Dsc, or the source paragraph of
//! `debian/control` on a Source tree, and only acts on pkg-perl packages.

use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;

use super::vcs::strip_source_prefix;

/// The mailing list of pkg-perl packages. Lintian matches it unanchored and
/// case-sensitively against the Maintainer.
const PKG_PERL_LIST: &str = "pkg-perl-maintainers@lists.alioth.debian.org";

/// The paragraph the Checks read: the Dsc's, else the source paragraph of
/// `debian/control`. `None` when neither exists.
pub(super) fn source_fields<'a>(ctx: &'a SourceContext<'_>) -> Option<&'a ControlParagraph> {
    match ctx.dsc() {
        Some(dsc) => Some(dsc.fields()),
        None => ctx.debian_control().map(|control| control.source()),
    }
}

/// Whether the Maintainer names the pkg-perl mailing list.
pub(super) fn is_pkg_perl(fields: &ControlParagraph) -> bool {
    fields
        .unfolded_value("Maintainer")
        .is_some_and(|maintainer| maintainer.contains(PKG_PERL_LIST))
}

/// A `Vcs-*` field `name` as lintian's `declares` and `literal_name` see it:
/// the name as written, and the unfolded value. Matching ignores case. On a
/// Source tree an `XS-` style field counts under its plain name, as the
/// Dsc has it (see `vcs::vcs_field_value`).
pub(super) fn vcs_field(
    fields: &ControlParagraph,
    name: &str,
    source_tree: bool,
) -> Option<(String, String)> {
    if let Some(key) = fields.names().find(|key| key.eq_ignore_ascii_case(name)) {
        return Some((key.to_string(), fields.unfolded_value(key)?));
    }
    if !source_tree {
        return None;
    }
    let (key, plain) = fields.names().find_map(|key| {
        strip_source_prefix(key)
            .filter(|plain| plain.eq_ignore_ascii_case(name))
            .map(|plain| (key, plain))
    })?;
    Some((plain.to_string(), fields.unfolded_value(key)?))
}
