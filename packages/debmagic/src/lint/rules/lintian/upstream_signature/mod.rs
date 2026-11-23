//! Rules for lintian's `upstream-signature` Check.
//!
//! The Check looks at the orig tarballs of a Source package and their
//! signature files, which sit next to the `.dsc` (the sibling files), and at
//! the signing keys the patched `debian/` ships. Each Tag is one Rule; the
//! findings are computed once here and each Rule reports its own Tag.

use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::data;
use crate::lint::source_package::{strip_debian_revision, strip_epoch};

mod ln1434_concatenated_upstream_signatures;

pub(crate) use ln1434_concatenated_upstream_signatures::ConcatenatedUpstreamSignatures;

const SIGNING_KEY_FILENAMES: &str = include_str!("../../../data/common/signing-key-filenames");

/// One finding of the Check: the Tag it belongs to and its extra text.
pub(super) struct Finding {
    pub tag: &'static str,
    pub extra: String,
}

/// The Check's findings for a Source package, in lintian's order, for the Tag
/// `tag`. A Source tree has no sibling files, so it has none.
pub(super) fn findings(ctx: &mut SourceContext<'_>, tag: &str) -> Vec<Finding> {
    let mut all = Vec::new();
    collect(ctx, &mut all);
    all.retain(|finding| finding.tag == tag);
    all
}

fn collect(ctx: &mut SourceContext<'_>, out: &mut Vec<Finding>) {
    let Some(siblings) = ctx.sibling_files() else {
        return;
    };
    let names = siblings.names().to_vec();

    // lintian's `@origtar`: sorted, not a signature, an orig tarball name.
    let orig_tarball = Regex::new(r"\.orig(?:-[A-Za-z\d-]+)?\.tar\.").expect("orig pattern");
    let origtars: Vec<&String> = names
        .iter()
        .filter(|name| !name.ends_with(".asc") && orig_tarball.is_match(name.as_bytes()))
        .collect();

    // Per orig tarball, its signatures: `<tarball>.asc`, and `<uncompressed>.asc`
    // where the uncompressed name ends at the last `.tar`.
    let mut signatures: Vec<(&String, Vec<String>)> = Vec::new();
    for filename in &origtars {
        let mut found = Vec::new();
        let uncompressed = filename.rfind(".tar").map(|index| &filename[..index + 4]);
        for tarball in std::iter::once(filename.as_str()).chain(uncompressed) {
            let signature = format!("{tarball}.asc");
            if siblings.contains(&signature) {
                found.push(signature);
            }
        }
        signatures.push((filename, found));
    }

    let tree = ctx.files();
    let signing_keys = tree.is_some_and(|tree| {
        data::lines(SIGNING_KEY_FILENAMES).any(|name| {
            tree.get(format!("debian/{name}"))
                .and_then(|entry| follow_symlinks(tree, entry))
                .is_some()
        })
    });
    let gittag = tree
        .and_then(|tree| {
            let entry = tree.get("debian/watch")?;
            let watch = follow_symlinks(tree, entry)?;
            tree.read(watch).ok()
        })
        .is_some_and(|bytes| GITTAG.is_match(&bytes));
    let repacked = repacked(ctx);

    // Orig tarballs should be signed if upstream's public key is present.
    if signing_keys && !repacked && !gittag {
        for (filename, found) in &signatures {
            if found.is_empty() {
                out.push(Finding {
                    tag: "orig-tarball-missing-upstream-signature",
                    extra: (*filename).clone(),
                });
            }
        }
    }

    let all_signatures = signatures.iter().flat_map(|(_, found)| found);
    for signature in all_signatures {
        let Some(contents) = siblings.read(signature) else {
            continue;
        };
        if ARMORED_HEADER.is_match(&contents) {
            if DOUBLY_ARMORED.is_match(&contents) {
                out.push(finding("doubly-armored-upstream-signature", signature));
            } else {
                out.push(finding("explicitly-armored-upstream-signature", signature));
            }
            let spurious: Vec<String> = SPURIOUS_FIELD
                .captures_iter(&contents)
                .map(|captures| String::from_utf8_lossy(&captures[1]).into_owned())
                .collect();
            if !spurious.is_empty() {
                let mut extra = signature.clone();
                for field in spurious {
                    extra.push(' ');
                    extra.push_str(&field);
                }
                out.push(Finding {
                    tag: "spurious-fields-in-upstream-signature",
                    extra,
                });
            }
        }
        if CONCATENATED.is_match(&contents) {
            out.push(finding("concatenated-upstream-signatures", signature));
        }
    }
}

fn finding(tag: &'static str, signature: &str) -> Finding {
    Finding {
        tag,
        extra: signature.to_string(),
    }
}

/// lintian's `$PKGREPACK_REGEX` on the upstream version of the `.dsc`.
fn repacked(ctx: &SourceContext<'_>) -> bool {
    static REPACK: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(dfsg|debian|ds|repack)").expect("repack pattern"));
    let version = ctx
        .dsc()
        .and_then(|dsc| dsc.fields().get("Version"))
        .unwrap_or_default()
        .trim();
    let upstream = strip_debian_revision(strip_epoch(version));
    REPACK.is_match(upstream)
}

/// `pgpmode=gittag` (uscan v4) or `pgp-mode: gittag` (v5, any case): uscan
/// never matches the signature then.
static GITTAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"pgpmode=gittag|(?i:pgp-?mode: gittag)").expect("gittag pattern"));

static ARMORED_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^-----BEGIN PGP ARMORED FILE-----").expect("armor pattern"));

/// `gpg --enarmor` output armored a second time: the base64 of an armored block.
static DOUBLY_ARMORED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^LS0tLS1CRUd").expect("doubly armored pattern"));

static SPURIOUS_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\n([^:\n]+):").expect("spurious field pattern"));

static CONCATENATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)(?:-----BEGIN PGP SIGNATURE-----[^-]*-----END PGP SIGNATURE-----\s*){2,}")
        .expect("concatenated pattern")
});
