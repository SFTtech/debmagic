use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::sfnt::SfntMagic;

pub struct OpentypeFontWrongFilename;

declare_rule! {
    /// The package installs an OpenType font whose name does not end in `.otf`.
    ///
    /// Walks Data files (regular files and hardlinks) that `file(1)` would
    /// call `OpenType font data` ([`SfntMagic::OpenType`]). The name is
    /// compared case-insensitively. Extra is empty; the path is the Location.
    OpentypeFontWrongFilename,
    code = "LN0784",
    tag = "opentype-font-wrong-filename",
    default_selected = true,
    experimental = false,
}

/// The name ends in `.otf`, ignoring ASCII case.
fn has_otf_suffix(path: &[u8]) -> bool {
    path.len() >= 4 && path[path.len() - 4..].eq_ignore_ascii_case(b".otf")
}

impl BinaryPackageRule for OpentypeFontWrongFilename {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        {
            if has_otf_suffix(entry.path().as_os_str().as_encoded_bytes()) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if SfntMagic::from_bytes(&bytes) == Some(SfntMagic::OpenType) {
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
    use crate::lint::sfnt::testing::sfnt;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, bytes: Vec<u8>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::from_bytes(bytes))])
    }

    fn otf() -> Vec<u8> {
        sfnt(b"OTTO", &[(b"cmap", &[0; 4])])
    }

    #[test]
    fn suffix() {
        assert!(has_otf_suffix(b"a.otf"));
        assert!(has_otf_suffix(b"a.OTF"));
        assert!(has_otf_suffix(b".otf"));
        assert!(!has_otf_suffix(b"a.ttf"));
        assert!(!has_otf_suffix(b"otf"));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/fonts/Named.otf", otf()),
            data("usr/share/fonts/Named.OTF", otf()),
            // Only the OpenType signature selects a file, not its name.
            data("usr/share/fonts/Text.ttf", b"not a font\n".to_vec()),
            data("usr/share/fonts/Text.txt", b"not a font\n".to_vec()),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Link.ttf".to_string(),
                TestFile::symlink("Named.otf"),
            )]),
        ];
        let fail = vec![
            data("usr/share/fonts/IsOTF.ttf", otf()),
            data("usr/share/fonts/NoSuffix", otf()),
            BinaryPackageCase::from(vec![(
                "usr/share/fonts/Hardlink.pfb".to_string(),
                TestFile::from_bytes(otf()).hardlink(),
            )]),
            data("usr/share/fonts/Udeb.ttf", otf()).udeb(),
        ];
        BinaryPackageTester::new(OpentypeFontWrongFilename, pass, fail).test_and_snapshot();
    }
}
