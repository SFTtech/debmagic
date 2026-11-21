use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::sfnt::SfntMagic;

pub struct TruetypeFontWrongFilename;

declare_rule! {
    /// The package installs a TrueType font whose name does not end in `.ttf`.
    ///
    /// Walks Data files (regular files and hardlinks) that `file(1)` would
    /// call `TrueType Font data` ([`SfntMagic::TrueType`]; the signature
    /// alone selects a file, not its name). The suffix is compared
    /// case-insensitively. Symlinks are skipped. Udebs are checked too.
    /// Extra is empty; the path is the Location.
    TruetypeFontWrongFilename,
    code = "LN1092",
    tag = "truetype-font-wrong-filename",
    default_selected = true,
    experimental = false,
}

/// The name ends in `.ttf`, ignoring ASCII case.
fn has_ttf_suffix(path: &[u8]) -> bool {
    path.len() >= 4 && path[path.len() - 4..].eq_ignore_ascii_case(b".ttf")
}

impl BinaryPackageRule for TruetypeFontWrongFilename {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        {
            if has_ttf_suffix(entry.path().as_os_str().as_encoded_bytes()) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if SfntMagic::from_bytes(&bytes) == Some(SfntMagic::TrueType) {
                hits.push(entry.path().to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::sfnt::testing::truetype;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, bytes: Vec<u8>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::from_bytes(bytes))])
    }

    #[test]
    fn suffix() {
        assert!(has_ttf_suffix(b"a.ttf"));
        assert!(has_ttf_suffix(b"a.TTF"));
        assert!(has_ttf_suffix(b".ttf"));
        assert!(!has_ttf_suffix(b"a.otf"));
        assert!(!has_ttf_suffix(b"ttf"));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/fonts/Named.ttf", truetype(0)),
            data("usr/share/fonts/Named.TTF", truetype(0)),
            // Only the TrueType signature selects a file, not its name.
            data("usr/share/fonts/Text.otf", b"not a font\n".to_vec()),
            data("usr/share/fonts/Text.txt", b"not a font\n".to_vec()),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Link.otf".to_string(),
                TestFile::symlink("Named.ttf"),
            )]),
        ];
        let fail = vec![
            data("usr/share/fonts/IsTTF.otf", truetype(0)),
            data("usr/share/fonts/NoSuffix", truetype(0)),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Hardlink.pfb".to_string(),
                TestFile::from_bytes(truetype(0)).hardlink(),
            )]),
            data("usr/share/fonts/Udeb.ttc", truetype(0)).udeb(),
        ];
        BinaryPackageTester::new(TruetypeFontWrongFilename, pass, fail).test_and_snapshot();
    }
}
