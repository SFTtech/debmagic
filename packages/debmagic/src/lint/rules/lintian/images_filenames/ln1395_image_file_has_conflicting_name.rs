use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::content_type::ContentType;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct ImageFileHasConflictingName;

declare_rule! {
    /// A regular file or hardlink in the Data files whose content is an image
    /// of one format but whose name is the usual name of another image format
    /// (`.png` for a JPEG, say). Reported at info severity with the extra
    /// `(is <FORMAT>)`, naming the format the content has, and the file's path
    /// as the Location.
    ///
    /// The image formats are PNG, JPEG, GIF, TIFF, XPM, Netpbm and SVG, with the
    /// name suffixes lintian's `images/filenames` Check uses. A file whose
    /// content is none of these is not an image and is skipped.
    ///
    /// Differs from lintian: the content format is sniffed from the leading
    /// bytes in this Rule, not from `file(1)`. SVG is recognised by an XML or
    /// `<svg` start that has `<svg` within its first 8 KiB, which is looser than
    /// `file(1)`. A name of an unexpected kind (no good name for the content) is
    /// `image-file-has-unexpected-name`, which is not part of this Rule.
    ImageFileHasConflictingName,
    code = "LN1395",
    tag = "image-file-has-conflicting-name",
    default_selected = false,
    experimental = false,
}

/// The image formats lintian's `images/filenames` Check knows, in its order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Tiff,
    Xpm,
    Netpbm,
    Svg,
}

impl ImageFormat {
    /// lintian's `name`, as the hint's `(is ...)` extra spells it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Tiff => "TIFF",
            Self::Xpm => "XPM",
            Self::Netpbm => "Netpbm",
            Self::Svg => "SVG",
        }
    }

    /// lintian's `good_name` test on the whole path.
    pub(crate) fn has_good_name(self, name: &[u8]) -> bool {
        let pattern = match self {
            Self::Png => &*GOOD_PNG,
            Self::Jpeg => &*GOOD_JPEG,
            Self::Gif => &*GOOD_GIF,
            Self::Tiff => &*GOOD_TIFF,
            Self::Xpm => &*GOOD_XPM,
            Self::Netpbm => &*GOOD_NETPBM,
            Self::Svg => &*GOOD_SVG,
        };
        pattern.is_match(name)
    }

    /// Every format, in lintian's order.
    pub(crate) const ALL: [Self; 7] = [
        Self::Png,
        Self::Jpeg,
        Self::Gif,
        Self::Tiff,
        Self::Xpm,
        Self::Netpbm,
        Self::Svg,
    ];
}

static GOOD_PNG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:png|PNG)$").expect("valid regex"));
static GOOD_JPEG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:jpe?g|JPE?G)$").expect("valid regex"));
static GOOD_GIF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:gif|GIF)$").expect("valid regex"));
static GOOD_TIFF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:tiff?|TIFF?)$").expect("valid regex"));
static GOOD_XPM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:xpm|XPM)$").expect("valid regex"));
static GOOD_NETPBM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:p[bgpn]m|P[BGPN]M)$").expect("valid regex"));
static GOOD_SVG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:svg|SVG)$").expect("valid regex"));

/// The image format of `bytes` by its leading bytes, as `file(1)` names it.
/// Sniffed by [`ContentType`]; the non-image types have no format here.
pub(crate) fn image_format(bytes: &[u8]) -> Option<ImageFormat> {
    Some(match ContentType::sniff(bytes)? {
        ContentType::Png => ImageFormat::Png,
        ContentType::Jpeg => ImageFormat::Jpeg,
        ContentType::Gif => ImageFormat::Gif,
        ContentType::Tiff => ImageFormat::Tiff,
        ContentType::Xpm => ImageFormat::Xpm,
        ContentType::Netpbm => ImageFormat::Netpbm,
        ContentType::Svg => ImageFormat::Svg,
        ContentType::Bmp
        | ContentType::Pdf
        | ContentType::PostScript
        | ContentType::Matlab { .. } => {
            return None;
        }
    })
}

fn conflicting_paths(files: &FileTree) -> Vec<(PathBuf, ImageFormat)> {
    files
        .entries()
        .iter()
        .filter(|entry| entry.is_file())
        .filter_map(|entry| {
            let name = entry.path().as_os_str().as_bytes();
            let bytes = files.read(entry).ok()?;
            let ours = image_format(&bytes)?;
            if ours.has_good_name(name) {
                return None;
            }
            // Some other format's name fits: a conflict.
            ImageFormat::ALL
                .into_iter()
                .any(|other| other != ours && other.has_good_name(name))
                .then(|| (entry.path().to_path_buf(), ours))
        })
        .collect()
}

impl BinaryPackageRule for ImageFileHasConflictingName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, format) in conflicting_paths(files) {
            ctx.diagnostic(
                Diagnostic::warn(format!("(is {})", format.name()))
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";
    const JPEG: &[u8] = b"\xff\xd8\xffrest";
    const GIF: &[u8] = b"GIF89a rest";
    const TIFF: &[u8] = b"II*\0rest";
    const XPM: &[u8] = b"/* XPM */\nstatic char *x[] = {};\n";
    const PBM: &[u8] = b"P4\n# Created\n8 1\n\x00";
    const SVG: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\n";

    fn image(path: &str, content: &'static [u8]) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(content))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            image("usr/share/example/a.png", PNG),
            image("usr/share/example/a.PNG", PNG),
            image("usr/share/example/a.jpg", JPEG),
            image("usr/share/example/a.jpeg", JPEG),
            image("usr/share/example/a.gif", GIF),
            image("usr/share/example/a.tif", TIFF),
            image("usr/share/example/a.xpm", XPM),
            image("usr/share/example/a.pbm", PBM),
            image("usr/share/example/a.svg", SVG),
            // Not an image.
            image("usr/share/example/a.png", b"plain text"),
            // Not an image name, so no conflict.
            image("usr/share/example/a.txt", PNG),
            // Content is an image and name matches no format: unexpected-name's business.
            image("usr/share/example/a.dat", PNG),
            // A conflict under another directory, but a directory is not a file.
            BinaryPackageCase::from(vec![("usr/share/example/a.png", TestFile::directory())]),
        ];
        let fail = vec![
            image("usr/share/example/a.png", JPEG),
            image("usr/share/example/a.jpeg", GIF),
            image("usr/share/example/a.gif", TIFF),
            image("usr/share/example/a.tif", PBM),
            image("usr/share/example/a.xpm", SVG),
            image("usr/share/example/a.svg", XPM),
            image("usr/share/example/a.pnm", PNG),
            image("usr/share/example/a.jpg", PNG),
            // A hardlink is a file too.
            BinaryPackageCase::from(vec![
                ("usr/share/example/a.jpg", TestFile::from_bytes(PNG)),
                (
                    "usr/share/example/b.png",
                    TestFile::from_bytes(PNG).hardlink_to("usr/share/example/a.jpg"),
                ),
            ]),
        ];
        BinaryPackageTester::new(ImageFileHasConflictingName, pass, fail).test_and_snapshot();
    }
}
