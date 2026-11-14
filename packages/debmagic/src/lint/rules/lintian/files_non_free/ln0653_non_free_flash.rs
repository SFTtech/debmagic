use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonFreeFlash;

declare_rule! {
    /// The package ships a known non-free Flash player.
    ///
    /// A regular file named `dewplayer.swf` (also `dewplayer-<word>.swf`),
    /// `mp3player.swf`, or `flvplayer.swf`, ignoring case, below at least one
    /// directory. Packages whose `Section` starts with `non-free/`,
    /// `non-free-firmware/`, `restricted/` or `multiverse/` are exempt.
    /// Extra is empty. Udebs are checked too.
    NonFreeFlash,
    code = "LN0653",
    tag = "non-free-flash",
    default_selected = true,
    experimental = false,
}

static FLASH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/(?i:dewplayer(?:-\w+)?\.swf$)|/(?i:(?:mp3|flv)player\.swf$)")
        .expect("valid regex")
});
static NON_FREE_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:non-free|non-free-firmware|restricted|multiverse)/").expect("valid regex")
});

impl BinaryPackageRule for NonFreeFlash {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let section = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
            .filter(|section| !section.is_empty())
            .unwrap_or_else(|| "main".to_string());
        if NON_FREE_SECTION.is_match(&section) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| FLASH.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|path| (path.to_string(), TestFile::new("CWS")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&["usr/share/foo/other.swf"]),
            // Needs a directory component.
            tree(&["mp3player.swf"]),
            tree(&["usr/share/foo/mp3player.swf.bak"]),
            tree(&["usr/share/foo/xmp3player.swf"]),
            // Non-free packages may ship it.
            tree(&["usr/share/foo/mp3player.swf"]).field("Section", "non-free/web"),
            tree(&["usr/share/foo/mp3player.swf"]).field("Section", "multiverse/web"),
        ];
        let fail = vec![
            tree(&["usr/share/foo/mp3player.swf"]),
            tree(&["usr/share/foo/FLVPlayer.swf"]),
            tree(&[
                "usr/share/foo/dewplayer.swf",
                "usr/share/foo/dewplayer-slim.swf",
            ]),
            tree(&["usr/share/foo/mp3player.swf"]).field("Section", "contrib/web"),
            tree(&["usr/share/foo/mp3player.swf"]).udeb(),
        ];
        BinaryPackageTester::new(NonFreeFlash, pass, fail).test_and_snapshot();
    }
}
