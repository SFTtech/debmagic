use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::ln1395_image_file_has_conflicting_name::{ImageFormat, image_format};

pub struct ImageFileHasUnexpectedName;

declare_rule! {
    /// A regular file or hardlink in the Data files whose content is an image
    /// but whose name is the usual name of no image format, so that
    /// `image-file-has-conflicting-name` does not apply. Reported at pedantic
    /// severity with the extra `(is <FORMAT>)` and the file's path as the
    /// Location.
    ///
    /// The formats and name suffixes are those of lintian's `images/filenames`
    /// Check; the content format is sniffed as described in
    /// `image-file-has-conflicting-name` (LN1395), which this Rule shares.
    ImageFileHasUnexpectedName,
    code = "LN1396",
    tag = "image-file-has-unexpected-name",
    default_selected = false,
    experimental = false,
}

fn unexpected_paths(files: &FileTree) -> Vec<(PathBuf, ImageFormat)> {
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
            // Another format's name fits: that is a conflict, not unexpected.
            let conflict = ImageFormat::ALL
                .into_iter()
                .any(|other| other != ours && other.has_good_name(name));
            (!conflict).then(|| (entry.path().to_path_buf(), ours))
        })
        .collect()
}

impl BinaryPackageRule for ImageFileHasUnexpectedName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, format) in unexpected_paths(files) {
            ctx.diagnostic(
                Diagnostic::warn(format!("(is {})", format.name()))
                    .with_severity(Severity::Pedantic)
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
    const GIF: &[u8] = b"GIF89a rest";

    fn image(path: &str, content: &'static [u8]) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(content))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            image("usr/share/example/a.png", PNG),
            image("usr/share/example/a.gif", GIF),
            // A name of another format is a conflict, not unexpected.
            image("usr/share/example/a.jpg", PNG),
            // Not an image.
            image("usr/share/example/a.dat", b"plain text"),
            // A directory is not a file.
            BinaryPackageCase::from(vec![("usr/share/example/a.dat", TestFile::directory())]),
        ];
        let fail = vec![
            image("usr/share/example/a.dat", PNG),
            image("usr/share/example/image", GIF),
            image("usr/share/example/a.pnm.bak", PNG),
            // A udeb is checked too.
            image("usr/share/example/image", PNG).udeb(),
        ];
        BinaryPackageTester::new(ImageFileHasUnexpectedName, pass, fail).test_and_snapshot();
    }
}
