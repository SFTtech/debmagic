//! A Debian binary package (`.deb`, `.udeb`, `.ddeb`) as an ar archive, and
//! the members of the tarballs inside it.
//!
//! Nothing is validated beyond what reading needs: any ar archive opens,
//! whatever its members, and `debian-binary` is not checked. Deciding what a
//! missing or extra member means is the caller's business.

use std::ffi::OsStr;
use std::io::{Cursor, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::compression;

/// One member of the ar archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArMember {
    /// The identifier, decoded lossily, trimmed, and without a trailing `/`.
    pub name: String,
    /// The size the header gives.
    pub size: u64,
    pub bytes: Vec<u8>,
}

/// The members of a binary package's ar archive, in archive order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryPackage {
    pub members: Vec<ArMember>,
}

impl BinaryPackage {
    /// Read every member of the ar archive at `path`. A file without the ar
    /// magic, or a member that cannot be read, is an error.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        use std::io::{Seek, SeekFrom};

        let mut file = std::fs::File::open(path)
            .with_context(|| format!("opening {} failed", path.display()))?;
        let mut magic = [0u8; 8];
        if file.read_exact(&mut magic).is_err() || &magic != b"!<arch>\n" {
            bail!("{} is not an ar archive", path.display());
        }
        file.seek(SeekFrom::Start(0))
            .with_context(|| format!("rewinding {} failed", path.display()))?;
        let mut archive = ar::Archive::new(file);
        let mut members = Vec::new();
        while let Some(entry) = archive.next_entry() {
            let mut entry =
                entry.with_context(|| format!("reading ar member from {}", path.display()))?;
            let name = member_name(entry.header().identifier());
            let size = entry.header().size();
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("reading {name} from {}", path.display()))?;
            members.push(ArMember { name, size, bytes });
        }
        Ok(Self { members })
    }
}

fn member_name(identifier: &[u8]) -> String {
    String::from_utf8_lossy(identifier)
        .trim()
        .trim_end_matches('/')
        .to_string()
}

/// The members of one tarball, in archive order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TarContents {
    /// Members with a name that is more than `.`, `..` or slashes.
    pub members: Vec<TarMember>,
    /// Whether any stored name started with `/` (GNU tar strips the slash and
    /// warns once per archive).
    pub leading_slash: bool,
}

/// One tarball member, read from its header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarMember {
    /// The name after [`normalize_member_path`]; bytes that are not UTF-8
    /// stay as stored.
    pub path: PathBuf,
    pub kind: TarKind,
    pub mode: u32,
    /// The ids, `0` when the field is blank or not a number.
    pub uid: u64,
    pub gid: u64,
    /// The user and group names, as stored, when the header has them.
    pub user_name: Option<Vec<u8>>,
    pub group_name: Option<Vec<u8>>,
    pub mtime: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarKind {
    File(Vec<u8>),
    Directory,
    /// The link target as stored.
    Symlink(PathBuf),
    /// The link target as stored, not normalized.
    Hardlink(PathBuf),
    Fifo,
    CharDevice,
    BlockDevice,
    /// Any other entry type (a sparse file, a pax global header, ...).
    Other,
}

/// The members of the tarball `name` (its extension picks the
/// decompression; see [`compression::decoder`]) with bytes `compressed`.
pub fn read_tarball(name: &str, compressed: &[u8]) -> anyhow::Result<TarContents> {
    let plain = compression::decompress_by_name(name, compressed)?;
    let mut archive = tar::Archive::new(Cursor::new(plain));
    let mut contents = TarContents::default();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let stored = entry.path()?;
        if stored.as_os_str().as_bytes().starts_with(b"/") {
            contents.leading_slash = true;
        }
        let Some(path) = normalize_member_path(&stored) else {
            continue;
        };
        let header = entry.header();
        let entry_type = header.entry_type();
        let mode = header
            .mode()
            .with_context(|| format!("{} has no valid mode", path.display()))?;
        let uid = header.uid().unwrap_or(0);
        let gid = header.gid().unwrap_or(0);
        let user_name = header.username_bytes().map(<[u8]>::to_vec);
        let group_name = header.groupname_bytes().map(<[u8]>::to_vec);
        let mtime = header_mtime(header);
        let link_target = |entry: &tar::Entry<'_, _>, what: &str| -> anyhow::Result<PathBuf> {
            Ok(entry
                .link_name()?
                .with_context(|| format!("{what} {} has no target", path.display()))?
                .into_owned())
        };
        let kind = if entry_type.is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            TarKind::File(bytes)
        } else if entry_type.is_dir() {
            TarKind::Directory
        } else if entry_type.is_symlink() {
            TarKind::Symlink(link_target(&entry, "symlink")?)
        } else if entry_type.is_hard_link() {
            TarKind::Hardlink(link_target(&entry, "hardlink")?)
        } else if entry_type.is_fifo() {
            TarKind::Fifo
        } else if entry_type.is_character_special() {
            TarKind::CharDevice
        } else if entry_type.is_block_special() {
            TarKind::BlockDevice
        } else {
            TarKind::Other
        };
        contents.members.push(TarMember {
            path,
            kind,
            mode,
            uid,
            gid,
            user_name,
            group_name,
            mtime,
        });
    }
    Ok(contents)
}

/// The header's modification time, seconds since the epoch. `None` when it is
/// unreadable or does not fit in an `i64`.
pub fn header_mtime(header: &tar::Header) -> Option<i64> {
    header
        .mtime()
        .ok()
        .and_then(|mtime| i64::try_from(mtime).ok())
}

/// Strip `./` prefixes, one leading `/`, and trailing `/` from a member path.
/// Works on the stored bytes, so a name that is not UTF-8 stays as it is.
/// `None` when nothing but `.` or `..` is left.
pub fn normalize_member_path(path: &Path) -> Option<PathBuf> {
    let mut value = path.as_os_str().as_bytes();
    while let Some(stripped) = value.strip_prefix(b"./") {
        value = stripped;
    }
    if let Some(stripped) = value.strip_prefix(b"/") {
        value = stripped;
    }
    while let Some(stripped) = value.strip_suffix(b"/") {
        value = stripped;
    }
    if value.is_empty() || value == b"." || value == b".." {
        return None;
    }
    Some(PathBuf::from(OsStr::from_bytes(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_prefixes_and_keeps_raw_bytes() {
        assert_eq!(
            normalize_member_path(Path::new("././usr/bin/")),
            Some(PathBuf::from("usr/bin"))
        );
        assert_eq!(
            normalize_member_path(Path::new("/etc")),
            Some(PathBuf::from("etc"))
        );
        assert_eq!(normalize_member_path(Path::new("./")), None);
        assert_eq!(normalize_member_path(Path::new("..")), None);
        let raw = OsStr::from_bytes(b"caf\xe9");
        assert_eq!(
            normalize_member_path(Path::new(raw)).map(|path| path.into_os_string()),
            Some(raw.to_os_string())
        );
    }

    #[test]
    fn reads_members_of_a_plain_tarball() -> anyhow::Result<()> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        // tar-rs refuses to write an absolute name, so store it raw.
        let name = b"/usr/file";
        header.as_old_mut().name[..name.len()].copy_from_slice(name);
        header.set_size(5);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &b"bytes"[..])?;
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        link.set_mode(0o777);
        builder.append_link(&mut link, "./usr/link", "file")?;
        let contents = read_tarball("data.tar", &builder.into_inner()?)?;
        assert!(contents.leading_slash);
        assert_eq!(contents.members[0].path, PathBuf::from("usr/file"));
        assert_eq!(contents.members[0].kind, TarKind::File(b"bytes".to_vec()));
        assert_eq!(contents.members[0].mode, 0o644);
        assert_eq!(
            contents.members[1].kind,
            TarKind::Symlink(PathBuf::from("file"))
        );
        Ok(())
    }
}
