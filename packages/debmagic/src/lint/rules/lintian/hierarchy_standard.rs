//! Which hint lintian's `files/hierarchy/standard` Check gives an installed
//! file. The Check is one `if`/`elsif` chain, so an earlier branch hides the
//! later ones; `classify` follows that order for the tags that have a Rule.

use std::path::PathBuf;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hierarchy {
    EtcOpt,
    DirInUsrLocal,
    FileInUsrLocal,
    VarLock,
    VarRun,
    Run,
    VarWww,
    Opt,
    Home,
    Tmp,
    FileDirectlyInUsrShare,
    SubdirInUsrBin,
}

/// `name` is lintian's name: directories end in a slash.
fn after<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    name.strip_prefix(prefix)
}

/// `^prefix.` : at least one character after the prefix.
fn has_more(name: &str, prefix: &str) -> bool {
    after(name, prefix).is_some_and(|rest| !rest.is_empty())
}

/// `^prefix[^/]+/$`: a top-level directory below `prefix`.
fn is_subdir_of(name: &str, prefix: &str) -> bool {
    after(name, prefix)
        .and_then(|rest| rest.strip_suffix('/'))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

pub(super) fn classify(name: &str, kind: EntryKind, udeb: bool) -> Option<Hierarchy> {
    let is_dir = kind == EntryKind::Directory;
    let is_file = matches!(kind, EntryKind::RegularFile | EntryKind::Hardlink);
    if has_more(name, "etc/opt/") {
        return Some(Hierarchy::EtcOpt);
    }
    if after(name, "usr/local/")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| !c.is_whitespace())
    {
        return Some(if is_dir {
            Hierarchy::DirInUsrLocal
        } else {
            Hierarchy::FileInUsrLocal
        });
    }
    if after(name, "usr/share/").is_some_and(|rest| !rest.contains('/')) {
        return is_file.then_some(Hierarchy::FileDirectlyInUsrShare);
    }
    if name.starts_with("usr/bin/") {
        let hit = is_dir
            && has_more(name, "usr/bin/")
            && !name.starts_with("usr/bin/X11/")
            && !name.starts_with("usr/bin/mh/");
        return hit.then_some(Hierarchy::SubdirInUsrBin);
    }
    // Branches of the chain that stop without a Hierarchy hint.
    if (!udeb && is_subdir_of(name, "usr/"))
        || (!udeb && is_subdir_of(name, "var/"))
        || (!udeb && has_more(name, "var/lib/games/"))
    {
        return None;
    }
    if !udeb && has_more(name, "var/lock/") {
        return Some(Hierarchy::VarLock);
    }
    if !udeb && has_more(name, "var/run/") {
        return Some(Hierarchy::VarRun);
    }
    if !udeb && has_more(name, "run/") {
        return Some(Hierarchy::Run);
    }
    if after(name, "var/www/")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| !c.is_whitespace())
    {
        return Some(Hierarchy::VarWww);
    }
    if has_more(name, "opt/") {
        return Some(Hierarchy::Opt);
    }
    if name.starts_with("hurd/") || name.starts_with("servers/") {
        return None;
    }
    if has_more(name, "home/") || has_more(name, "root/") {
        return Some(Hierarchy::Home);
    }
    if has_more(name, "tmp/") || has_more(name, "var/tmp/") || has_more(name, "usr/tmp/") {
        return Some(Hierarchy::Tmp);
    }
    None
}

/// Data files paths that get `hierarchy`, with a trailing slash on directories.
pub(super) fn hierarchy_paths(
    ctx: &mut BinaryPackageContext<'_>,
    hierarchy: Hierarchy,
) -> Vec<PathBuf> {
    let udeb = ctx.is_udeb();
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            let is_dir = entry.kind() == EntryKind::Directory;
            let path = entry.path().to_str()?;
            let name = if is_dir {
                format!("{path}/")
            } else {
                path.to_string()
            };
            (classify(&name, entry.kind(), udeb) == Some(hierarchy)).then(|| PathBuf::from(name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Hierarchy::*;
    use super::*;

    #[test]
    fn earlier_branches_hide_later_ones() {
        assert_eq!(classify("var/run/", EntryKind::Directory, false), None);
        assert_eq!(
            classify("var/run/foo/", EntryKind::Directory, false),
            Some(VarRun)
        );
        assert_eq!(classify("var/run/foo/", EntryKind::Directory, true), None);
        assert_eq!(classify("usr/tmp/", EntryKind::Directory, false), None);
        assert_eq!(
            classify("usr/tmp/x", EntryKind::RegularFile, false),
            Some(Tmp)
        );
        assert_eq!(classify("usr/tmp/", EntryKind::Directory, true), None);
        assert_eq!(classify("usr/local/", EntryKind::Directory, false), None);
        assert_eq!(
            classify("usr/local/x/", EntryKind::Directory, false),
            Some(DirInUsrLocal)
        );
        assert_eq!(
            classify("usr/local/x", EntryKind::RegularFile, false),
            Some(FileInUsrLocal)
        );
        assert_eq!(classify("var/www/", EntryKind::Directory, false), None);
        assert_eq!(
            classify("var/www/x", EntryKind::RegularFile, false),
            Some(VarWww)
        );
        assert_eq!(
            classify("root/x", EntryKind::RegularFile, false),
            Some(Home)
        );
        assert_eq!(
            classify("usr/share/x", EntryKind::RegularFile, false),
            Some(FileDirectlyInUsrShare)
        );
        assert_eq!(classify("usr/share/x", EntryKind::Symlink, false), None);
        assert_eq!(classify("usr/share/x/", EntryKind::Directory, false), None);
        assert_eq!(
            classify("usr/bin/x/", EntryKind::Directory, false),
            Some(SubdirInUsrBin)
        );
        assert_eq!(classify("usr/bin/X11/", EntryKind::Directory, false), None);
        assert_eq!(classify("usr/bin/x", EntryKind::RegularFile, false), None);
        assert_eq!(classify("hurd/x", EntryKind::RegularFile, false), None);
    }
}
