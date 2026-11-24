use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

use super::owner_exceptions::is_var_games_path;

pub struct NonStandardFilePerm;

declare_rule! {
    /// A non-executable file whose mode is not `0644`.
    ///
    /// Walks Data files regular files and hardlinks that are neither
    /// executable nor setuid or setgid. Reported with the extra
    /// `<mode> != 0644`; the path is the Location. These are left to other
    /// Rules, as lintian does: `etc/sudoers.d/` files (LN0944), `usr/lib/**.ali`
    /// files that are not `0444` (LN0945), `etc/backup.d/` files at `0600`,
    /// and game data below `var/games/` or `var/lib/games/` at `0664` owned
    /// by `root/games`. Game data with another owner is reported. Udebs are
    /// checked too.
    ///
    /// Where we differ from lintian: none. Lintian's
    /// `Screen/Toolchain/Gnat/AliReadOnly.pm` masks the hint for a read-only
    /// `usr/lib/<dir>/ada/adalib/<dir>/<name>.ali` in a `-dev` binary or udeb
    /// whose strong relation (Pre-Depends and Depends) satisfies `gnat`; that
    /// is modelled here for the `.ali` files at `0444`, the only ones this Rule
    /// reports.
    NonStandardFilePerm,
    code = "LN0947",
    tag = "non-standard-file-perm",
    default_selected = true,
    experimental = false,
}

const STANDARD_FILE: u32 = 0o644;
const WORLD_READABLE: u32 = 0o444;
const BACKUP_NINJA_FILE: u32 = 0o600;
const GAME_DATA: u32 = 0o664;
const SETUID_OR_SETGID: u32 = 0o6000;

/// lintian's `Screen/Toolchain/Gnat/AliReadOnly.pm` path: the `\b` is the
/// word boundary after `ali`, so `.alix` does not match.
static GNAT_ALI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/[^/]+/ada/adalib/[^/]+/[^/]+\.ali\b").expect("valid regex")
});

impl BinaryPackageRule for NonStandardFilePerm {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let gnat_dev =
            ctx.package_name().ends_with("-dev") && ctx.strong_relation().satisfies("gnat");
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && !entry.is_executable_file()
                    && entry.mode() & SETUID_OR_SETGID == 0
                    && entry.mode() != STANDARD_FILE
                    && !is_left_to_other_rules(entry)
                    && !(gnat_dev && is_gnat_ali_masked(entry))
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{mode:04o} != {STANDARD_FILE:04o}")).with_location(
                    Location {
                        path,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// lintian's `Screen/Toolchain/Gnat/AliReadOnly.pm` path test.
fn is_gnat_ali_masked(entry: &Entry) -> bool {
    GNAT_ALI.is_match(entry.path().as_os_str().as_bytes())
}

/// The paths and modes lintian's Check handles before its generic
/// `non-standard-file-perm` report, as its early returns do.
fn is_left_to_other_rules(entry: &Entry) -> bool {
    let path = entry.path().as_os_str().as_bytes();
    let mode = entry.mode();
    let is_ali = path.starts_with(b"usr/lib/") && path.ends_with(b".ali");
    // A `.ali` file at `0444` falls through to the generic report in lintian.
    (is_ali && mode != WORLD_READABLE)
        || path.starts_with(b"etc/sudoers.d/")
        || (mode == BACKUP_NINJA_FILE && path.starts_with(b"etc/backup.d/"))
        || (mode == GAME_DATA && entry.has_identity("root/games") && is_var_games_path(path))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file(mode: u32) -> TestFile {
        TestFile::new("data\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("usr/share/doc/pkg/README", file(0o644))]),
            // Executable or setuid/setgid: other Tags.
            tree(vec![("usr/bin/tool", file(0o755))]),
            tree(vec![("usr/bin/suid", file(0o4644))]),
            // Left to other Rules, as lintian does.
            tree(vec![("etc/sudoers.d/sudotest", file(0o604))]),
            tree(vec![("usr/lib/some-where/sample.ali", file(0o600))]),
            tree(vec![("etc/backup.d/job", file(0o600))]),
            tree(vec![(
                "var/games/pkg/score",
                file(0o664).owned_by("root", "games"),
            )]),
            tree(vec![(
                "var/lib/games/pkg/score",
                file(0o664).owned_by("root", "games"),
            )]),
            // Directories are not files.
            tree(vec![(
                "usr/share/doc/pkg",
                TestFile::directory().mode(0o755),
            )]),
        ];
        let fail = vec![
            tree(vec![("usr/share/doc/binary/read-only", file(0o444))]),
            tree(vec![("usr/share/doc/binary/world-write", file(0o666))]),
            tree(vec![("usr/share/doc/binary/owner-only", file(0o600))]),
            tree(vec![("etc/backup.d/other", file(0o640))]),
            // Game data owned by root/root (the fixture default) or someone else.
            tree(vec![("var/games/pkg/score", file(0o664))]),
            tree(vec![(
                "var/games/pkg/score",
                file(0o664).owned_by("root", "staff"),
            )]),
            tree(vec![(
                "var/lib/games/pkg/score",
                file(0o664).owned_by("games", "games"),
            )]),
            // Game data outside the owner-specific paths, or other modes there.
            tree(vec![(
                "var/games/pkg/other",
                file(0o640).owned_by("root", "games"),
            )]),
            tree(vec![("var/games/pkg/other", file(0o640))]),
            tree(vec![("var/lib/pkg/score", file(0o664))]),
            // An .ali file at 0444 falls through to the generic report.
            tree(vec![("usr/lib/some-where/sample.ali", file(0o444))]),
            tree(vec![("usr/share/doc/binary/hard", file(0o444).hardlink())]),
            tree(vec![("usr/share/doc/binary/udeb", file(0o444))]).udeb(),
        ];
        BinaryPackageTester::new(NonStandardFilePerm, pass, fail).test_and_snapshot();
    }

    fn ali_at_0444(path: &str) -> (&str, TestFile) {
        (path, file(0o444))
    }

    /// lintian `Screen/Toolchain/Gnat/AliReadOnly.pm` masks the hint for a
    /// read-only `.ali` under `usr/lib/<dir>/ada/adalib/<dir>/` in a `-dev` binary
    /// or udeb whose strong relation satisfies `gnat`. Only that exact depth and
    /// the `.ali` suffix count, and the mask needs the strong relation.
    #[test]
    fn gnat_ali_files_are_masked_in_gnat_dev_packages() {
        let gnat_dev = |fields: &[(&str, &str)]| {
            let mut case = tree(vec![ali_at_0444(
                "usr/lib/x86_64-linux-gnu/ada/adalib/foo/foo.ali",
            )])
            .package("libfoo-dev");
            for (name, value) in fields {
                case = case.field(*name, *value);
            }
            case
        };
        let pass = vec![
            gnat_dev(&[("Depends", "gnat")]),
            gnat_dev(&[("Depends", "gnat (>= 2)")]),
            gnat_dev(&[("Pre-Depends", "gnat")]),
            // A udeb of a -dev package is masked too.
            gnat_dev(&[("Depends", "gnat")]).udeb(),
        ];
        let fail = vec![
            // Not a -dev package.
            tree(vec![ali_at_0444("usr/lib/x/ada/adalib/foo/foo.ali")])
                .package("libfoo")
                .field("Depends", "gnat"),
            // No strong dependency on gnat: Recommends is not strong, and an
            // alternative does not satisfy the name.
            gnat_dev(&[("Recommends", "gnat")]),
            gnat_dev(&[("Depends", "gnat | gcc")]),
            gnat_dev(&[]),
            // The path must be exactly usr/lib/<dir>/ada/adalib/<dir>/<name>.ali.
            tree(vec![ali_at_0444("usr/lib/x/ada/adalib/foo.ali")])
                .package("libfoo-dev")
                .field("Depends", "gnat"),
            tree(vec![ali_at_0444("usr/lib/x/ada/adalib/foo/bar/foo.ali")])
                .package("libfoo-dev")
                .field("Depends", "gnat"),
            tree(vec![ali_at_0444("usr/lib/x/ada/other/foo/foo.ali")])
                .package("libfoo-dev")
                .field("Depends", "gnat"),
            // The suffix must be `.ali` and end the name's word.
            tree(vec![ali_at_0444("usr/lib/x/ada/adalib/foo/foo.alix")])
                .package("libfoo-dev")
                .field("Depends", "gnat"),
            // A sibling that is not an .ali is still reported.
            tree(vec![
                ali_at_0444("usr/lib/x/ada/adalib/foo/foo.ali"),
                ("usr/lib/x/ada/adalib/foo/foo.adb", file(0o444)),
            ])
            .package("libfoo-dev")
            .field("Depends", "gnat")
            .udeb(),
        ];
        BinaryPackageTester::new(NonStandardFilePerm, pass, fail).test();
    }
}
