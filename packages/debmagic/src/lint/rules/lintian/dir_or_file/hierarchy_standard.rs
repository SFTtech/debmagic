//! Order of lintian's `files/hierarchy/standard` Check.
//!
//! The Check is one `if`/`elsif` chain, so an earlier branch hides the later
//! ones. Each branch's predicate lives on its Rule. This module only keeps
//! that order, plus the stops that flag nothing.

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;

use super::flag::emit;
use super::ln0038_dir_or_file_in_home;
use super::ln0039_dir_or_file_in_tmp;
use super::ln0040_dir_or_file_in_opt;
use super::ln0041_dir_or_file_in_var_www;
use super::ln0042_dir_or_file_in_var_run;
use super::ln0043_dir_or_file_in_var_lock;
use super::ln0044_dir_or_file_in_run;
use super::ln0045_dir_or_file_in_etc_opt;
use super::ln0046_dir_in_usr_local;
use super::ln0047_file_in_usr_local;
use super::ln0048_file_directly_in_usr_share;
use super::ln0049_subdir_in_usr_bin;

pub(super) type Branch = fn(&str, EntryKind, bool) -> bool;

struct Link {
    /// When this returns, later links do not see the path.
    claims: Branch,
    /// Flags the Rule that owns `mark` when `claims` returned and `matches` is true.
    /// Absent for a stop that flags nothing. `mark` is that Rule's `MARK`.
    hit: Option<(&'static (), Branch)>,
}

impl Link {
    const fn rule(mark: &'static (), matches: Branch) -> Self {
        Self {
            claims: matches,
            hit: Some((mark, matches)),
        }
    }

    const fn wider(mark: &'static (), claims: Branch, matches: Branch) -> Self {
        Self {
            claims,
            hit: Some((mark, matches)),
        }
    }

    const fn stop(claims: Branch) -> Self {
        Self { claims, hit: None }
    }
}

/// Lintian's branch order. Earlier links hide later ones.
/// Each predicate and its `MARK` live on the Rule.
const LINKS: &[Link] = &[
    Link::rule(
        &ln0045_dir_or_file_in_etc_opt::MARK,
        ln0045_dir_or_file_in_etc_opt::matches,
    ),
    Link::rule(
        &ln0046_dir_in_usr_local::MARK,
        ln0046_dir_in_usr_local::matches,
    ),
    Link::rule(
        &ln0047_file_in_usr_local::MARK,
        ln0047_file_in_usr_local::matches,
    ),
    Link::wider(
        &ln0048_file_directly_in_usr_share::MARK,
        ln0048_file_directly_in_usr_share::claims,
        ln0048_file_directly_in_usr_share::matches,
    ),
    Link::wider(
        &ln0049_subdir_in_usr_bin::MARK,
        ln0049_subdir_in_usr_bin::claims,
        ln0049_subdir_in_usr_bin::matches,
    ),
    Link::stop(stop_non_udeb),
    Link::rule(
        &ln0043_dir_or_file_in_var_lock::MARK,
        ln0043_dir_or_file_in_var_lock::matches,
    ),
    Link::rule(
        &ln0042_dir_or_file_in_var_run::MARK,
        ln0042_dir_or_file_in_var_run::matches,
    ),
    Link::rule(
        &ln0044_dir_or_file_in_run::MARK,
        ln0044_dir_or_file_in_run::matches,
    ),
    Link::rule(
        &ln0041_dir_or_file_in_var_www::MARK,
        ln0041_dir_or_file_in_var_www::matches,
    ),
    Link::rule(
        &ln0040_dir_or_file_in_opt::MARK,
        ln0040_dir_or_file_in_opt::matches,
    ),
    Link::stop(stop_hurd),
    Link::rule(
        &ln0038_dir_or_file_in_home::MARK,
        ln0038_dir_or_file_in_home::matches,
    ),
    Link::rule(
        &ln0039_dir_or_file_in_tmp::MARK,
        ln0039_dir_or_file_in_tmp::matches,
    ),
];

/// `name` is lintian's name: directories end in a slash.
pub(in crate::lint::rules::lintian) fn after<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    name.strip_prefix(prefix)
}

/// `^prefix.` : at least one character after the prefix.
pub(in crate::lint::rules::lintian) fn has_more(name: &str, prefix: &str) -> bool {
    after(name, prefix).is_some_and(|rest| !rest.is_empty())
}

/// First character after `prefix` exists and is not whitespace.
pub(super) fn non_whitespace_after(name: &str, prefix: &str) -> bool {
    after(name, prefix)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|character| !character.is_whitespace())
}

/// `^prefix[^/]+/$`: a top-level directory below `prefix`.
fn is_subdir_of(name: &str, prefix: &str) -> bool {
    after(name, prefix)
        .and_then(|rest| rest.strip_suffix('/'))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

/// Non-udeb stops that flag nothing: a direct subdirectory of `usr/` or
/// `var/`, or anything under `var/lib/games/`.
fn stop_non_udeb(name: &str, _kind: EntryKind, udeb: bool) -> bool {
    !udeb
        && (is_subdir_of(name, "usr/")
            || is_subdir_of(name, "var/")
            || has_more(name, "var/lib/games/"))
}

fn stop_hurd(name: &str, _kind: EntryKind, _udeb: bool) -> bool {
    name.starts_with("hurd/") || name.starts_with("servers/")
}

fn lintian_name(path: &str, kind: EntryKind) -> String {
    if kind == EntryKind::Directory {
        format!("{path}/")
    } else {
        path.to_string()
    }
}

/// The Rule the chain selects for `name`, if it flags one. The pointer is that Rule's `MARK`.
fn winner(name: &str, kind: EntryKind, udeb: bool) -> Option<&'static ()> {
    for link in LINKS {
        if (link.claims)(name, kind, udeb) {
            return link
                .hit
                .and_then(|(mark, matches)| matches(name, kind, udeb).then_some(mark));
        }
    }
    None
}

fn same_mark(found: Option<&'static ()>, mark: &'static ()) -> bool {
    found.is_some_and(|found| std::ptr::eq(found, mark))
}

/// Emit hits the chain assigns to the Rule that owns `mark`. An earlier link
/// that claims the path leaves this Rule silent.
pub(in crate::lint::rules::lintian) fn flag_matching(
    ctx: &mut BinaryPackageContext<'_>,
    mark: &'static (),
) {
    let udeb = ctx.is_udeb();
    let Some(files) = ctx.data_files() else {
        return;
    };
    let paths: Vec<PathBuf> = files
        .entries()
        .iter()
        .filter_map(|entry| {
            let path = entry.path().to_str()?;
            let name = lintian_name(path, entry.kind());
            same_mark(winner(&name, entry.kind(), udeb), mark).then_some(PathBuf::from(name))
        })
        .collect();
    for path in paths {
        emit(ctx, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(name: &str, kind: EntryKind, udeb: bool, mark: &'static ()) -> bool {
        super::same_mark(super::winner(name, kind, udeb), mark)
    }

    #[test]
    fn earlier_branches_hide_later_ones() {
        assert!(winner("var/run/", EntryKind::Directory, false).is_none());
        assert!(selected(
            "var/run/foo/",
            EntryKind::Directory,
            false,
            &ln0042_dir_or_file_in_var_run::MARK
        ));
        assert!(winner("var/run/foo/", EntryKind::Directory, true).is_none());
        assert!(winner("usr/tmp/", EntryKind::Directory, false).is_none());
        assert!(selected(
            "usr/tmp/x",
            EntryKind::RegularFile,
            false,
            &ln0039_dir_or_file_in_tmp::MARK
        ));
        assert!(winner("usr/tmp/", EntryKind::Directory, true).is_none());
        assert!(winner("usr/local/", EntryKind::Directory, false).is_none());
        assert!(selected(
            "usr/local/x/",
            EntryKind::Directory,
            false,
            &ln0046_dir_in_usr_local::MARK
        ));
        assert!(selected(
            "usr/local/x",
            EntryKind::RegularFile,
            false,
            &ln0047_file_in_usr_local::MARK
        ));
        assert!(winner("var/www/", EntryKind::Directory, false).is_none());
        assert!(selected(
            "var/www/x",
            EntryKind::RegularFile,
            false,
            &ln0041_dir_or_file_in_var_www::MARK
        ));
        assert!(selected(
            "root/x",
            EntryKind::RegularFile,
            false,
            &ln0038_dir_or_file_in_home::MARK
        ));
        assert!(selected(
            "usr/share/x",
            EntryKind::RegularFile,
            false,
            &ln0048_file_directly_in_usr_share::MARK
        ));
        assert!(winner("usr/share/x", EntryKind::Symlink, false).is_none());
        assert!(winner("usr/share/x/", EntryKind::Directory, false).is_none());
        assert!(selected(
            "usr/bin/x/",
            EntryKind::Directory,
            false,
            &ln0049_subdir_in_usr_bin::MARK
        ));
        assert!(winner("usr/bin/X11/", EntryKind::Directory, false).is_none());
        assert!(winner("usr/bin/x", EntryKind::RegularFile, false).is_none());
        assert!(winner("hurd/x", EntryKind::RegularFile, false).is_none());
    }
}
