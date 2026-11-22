//! Lintian's known-field lists (`fields/unknown`).
//!
//! Debian is the vendor, so `@if-vendor-is-not debian` lines are absent. A
//! data-file name whose marker before the first `-` starts with `X` is stored
//! as the bare name (`XS-Autobuild` → `Autobuild`), matching the Check's
//! sanitising pass. Udeb names are stored as written.

use std::sync::LazyLock;

static SOURCE: LazyLock<Vec<String>> = LazyLock::new(|| {
    sanitize(include_str!("../../data/fields/source-fields"), |marker| {
        marker.starts_with(['X', 'x'])
    })
});
static BINARY: LazyLock<Vec<String>> = LazyLock::new(|| {
    sanitize(include_str!("../../data/fields/binary-fields"), |marker| {
        marker.len() >= 2 && marker[..2].eq_ignore_ascii_case("XB")
    })
});
static UDEB: LazyLock<Vec<String>> =
    LazyLock::new(|| entries(include_str!("../../data/fields/udeb-fields")));
/// Raw names as lintian's `common/source-fields` stores them: `X…` entries keep
/// their prefix. Used by `resembles`, which does not sanitise.
static SOURCE_RAW: LazyLock<Vec<String>> =
    LazyLock::new(|| entries(include_str!("../../data/common/source-fields")));
/// Raw names of `fields/binary-fields`, prefixes kept.
static BINARY_RAW: LazyLock<Vec<String>> =
    LazyLock::new(|| entries(include_str!("../../data/fields/binary-fields")));

pub(super) fn source_known(name: &str) -> bool {
    known(&SOURCE, name)
}

pub(super) fn binary_known(name: &str) -> bool {
    known(&BINARY, name)
}

pub(super) fn udeb_known(name: &str) -> bool {
    known(&UDEB, name)
}

/// Lintian's `resembles` on the source-field table: a case-insensitive match
/// against the raw entries, prefixes kept (`XS-Autobuild` matches only that
/// spelling, not the sanitised `Autobuild`).
#[allow(dead_code)]
pub(super) fn source_resembles(name: &str) -> bool {
    known(&SOURCE_RAW, name)
}

/// Lintian's `resembles` on the binary-field table; raw entries as for
/// [`source_resembles`].
#[allow(dead_code)]
pub(super) fn binary_resembles(name: &str) -> bool {
    known(&BINARY_RAW, name)
}

fn known(list: &[String], name: &str) -> bool {
    list.iter().any(|field| field.eq_ignore_ascii_case(name))
}

fn sanitize(text: &str, strip: impl Fn(&str) -> bool) -> Vec<String> {
    entries(text)
        .into_iter()
        .map(|field| match field.split_once('-') {
            Some((marker, bare)) if strip(marker) => bare.to_string(),
            _ => field,
        })
        .collect()
}

fn entries(text: &str) -> Vec<String> {
    crate::lint::data::lines(text)
        .filter(|line| !line.starts_with('@'))
        .map(str::to_string)
        .collect()
}

/// Names `dpkg-source` copies from the source paragraph into the `.dsc`.
///
/// `X…S…-Foo` becomes `Foo`. Other `X[SBC]-` fields stay out. An unprefixed
/// field is copied only when it is a known source field; dpkg drops the rest.
pub(super) fn dsc_names_from_source_paragraph<'a>(
    names: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let mut exported = Vec::new();
    for name in names {
        if let Some(bare) = custom_dsc_name(name) {
            exported.push(bare);
        } else if !is_custom(name) && source_known(name) {
            exported.push(name.to_string());
        }
    }
    exported
}

fn is_custom(name: &str) -> bool {
    custom_prefix(name).is_some()
}

fn custom_dsc_name(name: &str) -> Option<String> {
    let (dest, bare) = custom_prefix(name)?;
    dest.to_ascii_uppercase()
        .contains('S')
        .then(|| bare.to_string())
}

/// `X` plus zero or more of `S`/`B`/`C`, then `-`.
fn custom_prefix(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix(['X', 'x'])?;
    let (dest, bare) = rest.split_once('-')?;
    dest.chars()
        .all(|character| matches!(character, 'S' | 'B' | 'C' | 's' | 'b' | 'c'))
        .then_some((dest, bare))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resembles_keeps_prefixed_entries_raw() {
        // `XS-Autobuild` is stored bare by the sanitised list, but the raw
        // table keeps it prefixed, so only the raw lookup can see it.
        assert!(source_known("Autobuild"));
        assert!(!source_resembles("Autobuild"));
        assert!(source_resembles("xs-autobuild"));
    }

    #[test]
    fn resembles_is_case_insensitive_on_plain_names() {
        assert!(source_resembles("vcs-svn"));
        assert!(binary_resembles("Section"));
        assert!(!binary_resembles("Nonexistent-Field"));
    }
}
