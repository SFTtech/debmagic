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

pub(super) fn source_known(name: &str) -> bool {
    known(&SOURCE, name)
}

pub(super) fn binary_known(name: &str) -> bool {
    known(&BINARY, name)
}

pub(super) fn udeb_known(name: &str) -> bool {
    known(&UDEB, name)
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
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('@'))
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
