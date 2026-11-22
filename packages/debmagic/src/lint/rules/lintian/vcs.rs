/// Lintian's `%VCS_EXTRACT` keys, in check order.
pub(super) const PLATFORMS: &[&str] = &[
    "Browser", "Arch", "Bzr", "Cvs", "Darcs", "Hg", "Git", "Svn", "Mtn",
];

/// The unfolded value of a `Vcs-*` field `name`, or `None` when undeclared.
///
/// On a Source tree there is no Dsc, so the source paragraph of
/// `debian/control` stands in for it. `dpkg-source` copies an `X[SBC]*-`
/// field that carries `S` into the Dsc without the prefix, so `XS-Vcs-Browser`
/// counts as `Vcs-Browser` there. A Source package reads its Dsc as is.
pub(super) fn vcs_field_value(
    fields: &crate::lint::debian_control::ControlParagraph,
    name: &str,
    source_tree: bool,
) -> Option<String> {
    if let Some(value) = fields.unfolded_value(name) {
        return Some(value);
    }
    if !source_tree {
        return None;
    }
    let key = fields
        .names()
        .find(|key| strip_source_prefix(key).is_some_and(|rest| rest.eq_ignore_ascii_case(name)))?;
    fields.unfolded_value(key)
}

/// The name without `X`, one or more of `S`, `B`, `C` with `S` among them,
/// and the `-` of a source-only extension field such as `XS-` or `XSBC-`.
fn strip_source_prefix(key: &str) -> Option<&str> {
    let (prefix, rest) = key.split_once('-')?;
    let letters = prefix.strip_prefix(['X', 'x'])?;
    let upper = letters.to_ascii_uppercase();
    let valid = !upper.is_empty()
        && upper
            .chars()
            .all(|letter| matches!(letter, 'S' | 'B' | 'C'))
        && upper.contains('S');
    valid.then_some(rest)
}
