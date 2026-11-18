use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::sfnt::{Sfnt, SfntMagic};

pub struct OpentypeFontProhibitsInstallableEmbedding;

declare_rule! {
    /// The package installs an OpenType font that does not permit installable
    /// embedding.
    ///
    /// Walks Data files (regular files and hardlinks) that `file(1)` would
    /// call `OpenType font data` ([`SfntMagic::OpenType`]; the file name does
    /// not matter) and reads the OS/2 `fsType` as `Font::TTF` does. Extra is
    /// the restrictions in parentheses, in the order `never embed`,
    /// `preview/print only`, `edit only`, joined with `, `. The path is the
    /// Location. Fonts `Font::TTF` cannot open, and fonts without a readable
    /// `fsType`, are skipped.
    OpentypeFontProhibitsInstallableEmbedding,
    code = "LN0736",
    tag = "opentype-font-prohibits-installable-embedding",
    default_selected = true,
    experimental = false,
}

const NEVER_EMBED: u16 = 0x0002;
const PREVIEW_PRINT_ONLY: u16 = 0x0004;
const EDIT_ONLY: u16 = 0x0008;

/// Lintian's words for the restrictive bits of `fsType`, in its order.
fn restrictions(fs_type: u16) -> Vec<&'static str> {
    [
        (NEVER_EMBED, "never embed"),
        (PREVIEW_PRINT_ONLY, "preview/print only"),
        (EDIT_ONLY, "edit only"),
    ]
    .into_iter()
    .filter(|(flag, _)| fs_type & flag != 0)
    .map(|(_, words)| words)
    .collect()
}

impl BinaryPackageRule for OpentypeFontProhibitsInstallableEmbedding {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        {
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if SfntMagic::from_bytes(&bytes) != Some(SfntMagic::OpenType) {
                continue;
            }
            let Some(fs_type) = Sfnt::parse(&bytes).ok().and_then(|font| font.fs_type()) else {
                continue;
            };
            let restrictions = restrictions(fs_type);
            if !restrictions.is_empty() {
                hits.push((entry.path().to_path_buf(), restrictions.join(", ")));
            }
        }
        for (path, restrictions) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("({restrictions})")).with_location(Location {
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
    use crate::lint::sfnt::testing::{os2, sfnt, truetype};
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, bytes: Vec<u8>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::from_bytes(bytes))])
    }

    fn otf(fs_type: u16) -> Vec<u8> {
        sfnt(b"OTTO", &[(b"cmap", &[0; 4]), (b"OS/2", &os2(4, fs_type))])
    }

    #[test]
    fn restriction_words() {
        assert!(restrictions(0x0000).is_empty());
        assert!(restrictions(0x0101).is_empty());
        assert_eq!(
            restrictions(0x000e),
            ["never embed", "preview/print only", "edit only"]
        );
        assert_eq!(restrictions(0xfff4), ["preview/print only"]);
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Installable embedding.
            data("usr/share/fonts/Installable.otf", otf(0x0000)),
            // TrueType outlines are the truetype Check's.
            data("usr/share/fonts/NeverEmbed.ttf", truetype(0x0002)),
            // `file` does not call a `.otf` that is not an sfnt a font.
            data("usr/share/fonts/Text.otf", b"never embed\n".to_vec()),
            // Font::TTF 1.06 reads no fields from OS/2 version 5.
            data(
                "usr/share/fonts/Os2v5.otf",
                sfnt(b"OTTO", &[(b"OS/2", &os2(5, 0x0002))]),
            ),
            data(
                "usr/share/fonts/NoOs2.otf",
                sfnt(b"OTTO", &[(b"cmap", &[0; 4])]),
            ),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Link.otf".to_string(),
                TestFile::symlink("NeverEmbed.otf"),
            )]),
        ];
        let fail = vec![
            data("usr/share/fonts/NeverEmbed.otf", otf(0x0002)),
            data("usr/share/fonts/Printable.otf", otf(0x0004)),
            data("usr/share/fonts/Editable.otf", otf(0x0008)),
            data("usr/share/fonts/All.otf", otf(0x000e)),
            // Selected by content, not by name.
            data("usr/share/fonts/IsOTF.ttf", otf(0x0002)),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Hardlink.otf".to_string(),
                TestFile::from_bytes(otf(0x0008)).hardlink(),
            )]),
            data("usr/share/fonts/Udeb.otf", otf(0x0002)).udeb(),
        ];
        BinaryPackageTester::new(OpentypeFontProhibitsInstallableEmbedding, pass, fail)
            .test_and_snapshot();
    }
}
