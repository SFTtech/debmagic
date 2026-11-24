use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, bail};

use super::deb_members::{DebMember, DebMembers, Tarball};
use super::file_tree::Owner;
use super::timing;

/// In-process reader for a Binary package. Callers list and read Control files
/// and Data files; they do not hold the inner tarballs.
pub(crate) struct BinaryPackageReader {
    path: PathBuf,
    deb_members: DebMembers,
    /// Name and bytes of the Control and Data tarballs, `None` when the archive
    /// has no such member. Their contents are indexed lazily.
    control_tar: Option<(String, Vec<u8>)>,
    data_tar: Option<(String, Vec<u8>)>,
    control_index: OnceLock<Result<TarballIndex, String>>,
    data_index: OnceLock<Result<TarballIndex, String>>,
}

struct TarballIndex {
    #[allow(dead_code)]
    members: Vec<PathBuf>,
    files: HashMap<PathBuf, TarFile>,
    /// Every member that is not a regular file (directories, symlinks,
    /// hardlinks, fifos and devices), in archive order. Symlink targets are as
    /// stored; hardlink targets are normalized member paths.
    links_and_dirs: Vec<(PathBuf, TarMember)>,
    /// What GNU tar prints while unpacking the tarball (lintian's
    /// `unpack-messages`), in archive order, each message once.
    unpack_messages: Vec<String>,
}

/// GNU tar's warning for member names with a leading `/`: it strips the slash
/// and says so once per archive.
const LEADING_SLASH_WARNING: &str = "tar: Removing leading `/' from member names";

/// A regular file member: its bytes, the header's permission bits and owner.
struct TarFile {
    mode: u32,
    owner: Owner,
    mtime: Option<i64>,
    bytes: Vec<u8>,
}

/// A member that is not a regular file, with the header's permission bits and
/// owner.
struct TarMember {
    kind: MemberKind,
    mode: u32,
    owner: Owner,
    mtime: Option<i64>,
}

enum MemberKind {
    Directory,
    Symlink(PathBuf),
    Hardlink(PathBuf),
    Fifo,
    CharDevice,
    BlockDevice,
}

impl BinaryPackageReader {
    /// Open a Binary package. Any ar archive opens, whatever its members, so
    /// that `deb-format` can report a malformed one (`malformed-deb-archive`).
    /// A file without the ar magic is not a Binary package. A missing Control
    /// or Data tarball fails only when its contents are asked for.
    pub(crate) fn from_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        use std::io::{Seek, SeekFrom};

        let path = path.into();
        let mut file = std::fs::File::open(&path)
            .with_context(|| format!("opening {} failed", path.display()))?;
        let mut magic = [0u8; 8];
        if file.read_exact(&mut magic).is_err() || &magic != b"!<arch>\n" {
            bail!("{} is not an ar archive", path.display());
        }
        file.seek(SeekFrom::Start(0))
            .with_context(|| format!("rewinding {} failed", path.display()))?;
        let mut archive = ar::Archive::new(file);
        let mut control_tar = None;
        let mut data_tar = None;
        let mut deb_members: Vec<DebMember> = Vec::new();
        while let Some(entry) = archive.next_entry() {
            let mut entry =
                entry.with_context(|| format!("reading ar member from {}", path.display()))?;
            let name = ar_member_name(entry.header().identifier());
            let size = entry.header().size();
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("reading {name} from {}", path.display()))?;
            let first_debian_binary =
                name == "debian-binary" && !deb_members.iter().any(|m| m.name() == name);
            let mut member = DebMember::new(name.clone(), size);
            if first_debian_binary {
                member = member.with_contents(bytes.clone());
            } else if name.starts_with("control.tar") {
                if control_tar.is_none() {
                    control_tar = Some((name.clone(), bytes));
                }
            } else if name.starts_with("data.tar") && data_tar.is_none() {
                data_tar = Some((name.clone(), bytes));
            }
            deb_members.push(member);
        }
        Ok(Self {
            path,
            deb_members: DebMembers::new(deb_members),
            control_tar,
            data_tar,
            control_index: OnceLock::new(),
            data_index: OnceLock::new(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The ar members, in archive order.
    pub(crate) fn deb_members(&self) -> &DebMembers {
        &self.deb_members
    }

    /// Whether the archive has a member named `control.tar*`. Without one there
    /// are no Control files and no Installation control.
    pub(crate) fn has_control_tarball(&self) -> bool {
        self.control_tar.is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn list_control_files(&self) -> anyhow::Result<&[PathBuf]> {
        Ok(&self.control_index()?.members)
    }

    #[allow(dead_code)]
    pub(crate) fn list_data_files(&self) -> anyhow::Result<&[PathBuf]> {
        Ok(&self.data_index()?.members)
    }

    pub(crate) fn read_control_file(&self, path: &str) -> anyhow::Result<Option<&[u8]>> {
        Ok(lookup_file(self.control_index()?, path))
    }

    #[allow(dead_code)]
    pub(crate) fn read_data_file(&self, path: &str) -> anyhow::Result<Option<&[u8]>> {
        Ok(lookup_file(self.data_index()?, path))
    }

    /// The messages tar printed while unpacking `tarball`, in order. Empty when
    /// the tarball is missing or cannot be read: its failure is reported by the
    /// Files trees, not here.
    pub(crate) fn unpack_messages(&self, tarball: Tarball) -> Vec<String> {
        let index = match tarball {
            Tarball::Data => self.data_index(),
            Tarball::Control => self.control_index(),
        };
        index
            .map(|index| index.unpack_messages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn data_file_tree(&self) -> anyhow::Result<super::file_tree::FileTree> {
        Ok(file_tree(self.data_index()?))
    }

    /// The Control files File tree: every member of the Control tarball.
    pub(crate) fn control_file_tree(&self) -> anyhow::Result<super::file_tree::FileTree> {
        Ok(file_tree(self.control_index()?))
    }

    fn control_index(&self) -> anyhow::Result<&TarballIndex> {
        let Some((name, bytes)) = &self.control_tar else {
            bail!("{} has no control.tar member", self.path.display());
        };
        init_index(&self.control_index, name, bytes, self.path.as_path())
    }

    fn data_index(&self) -> anyhow::Result<&TarballIndex> {
        let Some((name, bytes)) = &self.data_tar else {
            bail!("{} has no data.tar member", self.path.display());
        };
        init_index(&self.data_index, name, bytes, self.path.as_path())
    }
}

fn file_tree(index: &TarballIndex) -> super::file_tree::FileTree {
    use super::file_tree::{Entry, FileTree};
    use super::file_type::FileType;

    let mut entries = Vec::with_capacity(index.files.len() + index.links_and_dirs.len());
    for (path, file) in &index.files {
        let file_type = FileType::from_bytes(&file.bytes);
        entries.push(
            Entry::regular_file(path, file_type, Some(file.bytes.clone()))
                .with_mode(file.mode)
                .with_owner(file.owner.clone())
                .with_mtime(file.mtime),
        );
    }
    for (path, member) in &index.links_and_dirs {
        let entry = match &member.kind {
            MemberKind::Directory => Entry::directory(path),
            MemberKind::Symlink(target) => Entry::symlink(path, target),
            MemberKind::Hardlink(target) => {
                let content = index.files.get(target).map(|file| file.bytes.clone());
                let file_type = content.as_deref().and_then(FileType::from_bytes);
                Entry::hardlink(path, file_type, content).with_hardlink_target(target)
            }
            MemberKind::Fifo => Entry::fifo(path),
            MemberKind::CharDevice => Entry::char_device(path),
            MemberKind::BlockDevice => Entry::block_device(path),
        };
        entries.push(
            entry
                .with_mode(member.mode)
                .with_owner(member.owner.clone())
                .with_mtime(member.mtime),
        );
    }
    FileTree::from_entries(None, entries)
}

fn init_index<'a>(
    cell: &'a OnceLock<Result<TarballIndex, String>>,
    name: &str,
    compressed: &[u8],
    archive_path: &Path,
) -> anyhow::Result<&'a TarballIndex> {
    // The result, errors included, is cached so concurrent callers index each
    // tarball once and all observe the same outcome.
    timing::init_once(cell, || {
        index_tarball(name, compressed)
            .with_context(|| format!("indexing {name} from {}", archive_path.display()))
            .map_err(|error| format!("{error:#}"))
    })
    .as_ref()
    .map_err(|message| anyhow::anyhow!("{message}"))
}

fn lookup_file<'a>(index: &'a TarballIndex, path: &str) -> Option<&'a [u8]> {
    let key = normalize_member_path(Path::new(path))?;
    index.files.get(&key).map(|file| file.bytes.as_slice())
}

fn ar_member_name(identifier: &[u8]) -> String {
    String::from_utf8_lossy(identifier)
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn index_tarball(name: &str, compressed: &[u8]) -> anyhow::Result<TarballIndex> {
    let tar_bytes = decompress_tar(name, compressed)?;
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    let mut members = Vec::new();
    let mut seen = HashSet::new();
    let mut files = HashMap::new();
    let mut links_and_dirs = Vec::new();
    let mut unpack_messages = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.as_os_str().as_encoded_bytes().starts_with(b"/")
            && !unpack_messages
                .iter()
                .any(|message| message == LEADING_SLASH_WARNING)
        {
            unpack_messages.push(LEADING_SLASH_WARNING.to_string());
        }
        let Some(normalized) = normalize_member_path(path.as_ref()) else {
            continue;
        };
        if seen.insert(normalized.clone()) {
            members.push(normalized.clone());
        }
        let entry_type = entry.header().entry_type();
        let mode = entry
            .header()
            .mode()
            .with_context(|| format!("{} has no valid mode", normalized.display()))?;
        let owner = header_owner(entry.header());
        let mtime = header_mtime(entry.header());
        let member = |kind| TarMember {
            kind,
            mode,
            owner: owner.clone(),
            mtime,
        };
        if entry_type.is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            files.insert(
                normalized,
                TarFile {
                    mode,
                    owner,
                    mtime,
                    bytes,
                },
            );
        } else if entry_type.is_dir() {
            links_and_dirs.push((normalized, member(MemberKind::Directory)));
        } else if entry_type.is_symlink() {
            let target = entry
                .link_name()?
                .with_context(|| format!("symlink {} has no target", normalized.display()))?
                .into_owned();
            links_and_dirs.push((normalized, member(MemberKind::Symlink(target))));
        } else if entry_type.is_hard_link() {
            let target = entry
                .link_name()?
                .with_context(|| format!("hardlink {} has no target", normalized.display()))?;
            let Some(target) = normalize_member_path(target.as_ref()) else {
                continue;
            };
            links_and_dirs.push((normalized, member(MemberKind::Hardlink(target))));
        } else if entry_type.is_fifo() {
            links_and_dirs.push((normalized, member(MemberKind::Fifo)));
        } else if entry_type.is_character_special() {
            links_and_dirs.push((normalized, member(MemberKind::CharDevice)));
        } else if entry_type.is_block_special() {
            links_and_dirs.push((normalized, member(MemberKind::BlockDevice)));
        }
    }
    Ok(TarballIndex {
        members,
        files,
        links_and_dirs,
        unpack_messages,
    })
}

/// The header's numeric ids and, when it has them, user and group names. An id
/// field that is blank or not a number reads as `0`, lintian's default.
fn header_owner(header: &tar::Header) -> Owner {
    let name = |bytes: Option<&[u8]>| bytes.map(|name| String::from_utf8_lossy(name).into_owned());
    Owner::new(
        header.uid().unwrap_or(0),
        header.gid().unwrap_or(0),
        name(header.username_bytes()),
        name(header.groupname_bytes()),
    )
}

/// The header's modification time, seconds since the epoch. `None` when it is
/// unreadable or does not fit in an `i64`.
pub(crate) fn header_mtime(header: &tar::Header) -> Option<i64> {
    header
        .mtime()
        .ok()
        .and_then(|mtime| i64::try_from(mtime).ok())
}

/// Strip `./` prefixes, one leading `/`, and trailing `/` from a member path.
/// Works on the stored bytes, so a name that is not UTF-8 stays as it is.
pub(crate) fn normalize_member_path(path: &Path) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

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
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(value)))
}

fn decompress_tar(name: &str, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(bytes);
    if name.ends_with(".gz") {
        let mut decoder = flate2::read::GzDecoder::new(cursor);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        return Ok(out);
    }
    if name.ends_with(".bz2") {
        let mut decoder = bzip2::read::BzDecoder::new(cursor);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        return Ok(out);
    }
    if name.ends_with(".xz") {
        let mut decoder = xz2::read::XzDecoder::new(cursor);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        return Ok(out);
    }
    if name.ends_with(".zst") {
        let mut decoder = zstd::stream::read::Decoder::new(cursor)?;
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        return Ok(out);
    }
    if name.ends_with(".tar") {
        return Ok(bytes.to_vec());
    }
    bail!("unsupported tarball {name}");
}

/// Lintian's `guess_name`: basename, drop extension, drop after the first `_`.
pub(crate) fn guess_package_name(path: &Path) -> String {
    let mut guess = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if let Some((stem, _)) = guess.rsplit_once('.') {
        guess = stem.to_string();
    }
    if let Some((before, _)) = guess.split_once('_') {
        guess = before.to_string();
    }
    guess
}

#[cfg(test)]
pub(crate) fn write_test_deb(path: &Path, control: &str) -> anyhow::Result<()> {
    write_test_deb_with_files(path, &[("control", control.as_bytes())], &[])
}

#[cfg(test)]
pub(crate) fn write_test_deb_with_files(
    path: &Path,
    control_files: &[(&str, &[u8])],
    data_files: &[(&str, &[u8])],
) -> anyhow::Result<()> {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    fn gzip_tar(files: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (member, contents) in files {
                let mut header = tar::Header::new_gnu();
                header.set_path(*member)?;
                header.set_size(contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, *contents)?;
            }
            builder.finish()?;
        }
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_bytes)?;
        Ok(encoder.finish()?)
    }

    let control_tar_gz = gzip_tar(control_files)?;
    let data_tar_gz = gzip_tar(data_files)?;

    let file = std::fs::File::create(path)?;
    let mut builder = ar::Builder::new(file);
    let debian_binary = b"2.0\n";
    let mut header = ar::Header::new(b"debian-binary".to_vec(), debian_binary.len() as u64);
    builder.append(&header, &mut Cursor::new(&debian_binary[..]))?;
    header = ar::Header::new(b"control.tar.gz".to_vec(), control_tar_gz.len() as u64);
    builder.append(&header, &mut Cursor::new(&control_tar_gz))?;
    header = ar::Header::new(b"data.tar.gz".to_vec(), data_tar_gz.len() as u64);
    builder.append(&header, &mut Cursor::new(&data_tar_gz))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_control_from_minimal_deb() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb(&path, "Package: example\nVersion: 1.0\n")?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let bytes = reader.read_control_file("control")?;
        assert_eq!(
            std::str::from_utf8(bytes.expect("control file"))?,
            "Package: example\nVersion: 1.0\n"
        );
        Ok(())
    }

    #[test]
    fn missing_required_ar_member_is_an_error() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("empty.deb");
        std::fs::write(&path, b"")?;
        let error = BinaryPackageReader::from_path(&path)
            .err()
            .expect("empty archive");
        assert!(
            error.to_string().contains("debian-binary")
                || error.to_string().contains("control.tar")
                || error.to_string().contains("data.tar")
                || error.to_string().contains("ar"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn missing_data_tarball_opens_and_fails_only_for_data_files() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("control-only.deb");
        write_ar_members(
            &path,
            &[
                ("debian-binary", b"2.0\n".as_slice()),
                ("control.tar.gz", &gzip_empty_tar()?),
            ],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert!(reader.has_control_tarball());
        assert_eq!(reader.deb_members().iter().count(), 2);
        let error = reader.data_file_tree().err().expect("no data.tar");
        assert!(error.to_string().contains("data.tar"), "{error}");
        Ok(())
    }

    #[test]
    fn archive_without_control_tarball_opens_with_no_control_files() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("bare.deb");
        write_ar_members(&path, &[("debian-binary", b"3.0\n".as_slice())])?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert!(!reader.has_control_tarball());
        assert!(reader.control_file_tree().is_err());
        let control = crate::lint::installation_control::InstallationControl::from_reader(&reader)?;
        assert!(control.fields().get("Package").is_none());
        Ok(())
    }

    #[test]
    fn data_entries_carry_the_header_mtime() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        // `write_test_deb_with_files` leaves the tar header's mtime at 0.
        write_test_deb_with_files(
            &path,
            &[("control", b"Package: example\n")],
            &[("usr/bin/foo", b"x")],
        )?;
        let tree = BinaryPackageReader::from_path(&path)?.data_file_tree()?;
        let foo = tree.get("usr/bin/foo").expect("foo");
        assert_eq!(foo.mtime(), Some(0));
        assert_eq!(foo.mtime_date().as_deref(), Some("1970-01-01"));
        Ok(())
    }

    #[test]
    fn debian_binary_contents_are_kept_for_the_format_version() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb(&path, "Package: example\n")?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let member = reader.deb_members().debian_binary().expect("debian-binary");
        assert_eq!(member.contents(), Some(b"2.0\n".as_slice()));
        assert_eq!(member.size(), 4);
        // Other members keep no contents.
        let other = reader.deb_members().iter().nth(1).expect("control");
        assert_eq!(other.contents(), None);
        Ok(())
    }

    #[test]
    fn a_file_without_the_ar_magic_is_not_a_binary_package() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("text.deb");
        std::fs::write(&path, b"not an archive at all")?;
        let error = BinaryPackageReader::from_path(&path).err().expect("not ar");
        assert!(error.to_string().contains("not an ar archive"), "{error}");
        Ok(())
    }

    #[test]
    fn lists_and_reads_control_and_data_files() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb_with_files(
            &path,
            &[
                ("control", b"Package: example\n"),
                ("./md5sums", b"deadbeef  usr/bin/foo\n"),
            ],
            &[
                ("./usr/bin/foo", b"#!/bin/sh\n"),
                ("usr/share/doc/example/copyright", b"Copyright\n"),
            ],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert_eq!(
            reader.list_control_files()?,
            [PathBuf::from("control"), PathBuf::from("md5sums")].as_slice()
        );
        assert_eq!(
            reader.list_data_files()?,
            [
                PathBuf::from("usr/bin/foo"),
                PathBuf::from("usr/share/doc/example/copyright")
            ]
            .as_slice()
        );
        assert_eq!(
            reader.read_control_file("md5sums")?,
            Some(b"deadbeef  usr/bin/foo\n".as_slice())
        );
        assert_eq!(
            reader.read_data_file("usr/bin/foo")?,
            Some(b"#!/bin/sh\n".as_slice())
        );
        assert_eq!(
            reader.read_data_file("./usr/bin/foo")?,
            reader.read_data_file("usr/bin/foo")?
        );
        assert!(reader.read_control_file("preinst")?.is_none());
        assert!(reader.read_data_file("usr/bin/missing")?.is_none());
        Ok(())
    }

    #[test]
    fn directories_and_symlinks_are_listed_but_not_readable() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_deb_with_special_data_members(&path)?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let listed = reader.list_data_files()?;
        assert!(listed.contains(&PathBuf::from("usr")), "{listed:?}");
        assert!(
            listed.contains(&PathBuf::from("usr/bin/link")),
            "{listed:?}"
        );
        assert!(reader.read_data_file("usr")?.is_none());
        assert!(reader.read_data_file("usr/bin/link")?.is_none());
        Ok(())
    }

    #[test]
    fn guess_name_drops_extension_and_underscore() {
        assert_eq!(
            guess_package_name(Path::new("fields-general-missing.deb")),
            "fields-general-missing"
        );
        assert_eq!(
            guess_package_name(Path::new("generic-empty_1.0_all.deb")),
            "generic-empty"
        );
        assert_eq!(guess_package_name(Path::new("foo.udeb")), "foo");
    }

    #[test]
    fn data_file_tree_keeps_hardlinks_and_backslashes() -> anyhow::Result<()> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let mut header = tar::Header::new_gnu();
            header.set_size(4);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, "usr/bin/a\\b", &b"data"[..])?;
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Link);
            link.set_size(0);
            link.set_mode(0o644);
            link.set_cksum();
            builder.append_link(&mut link, "usr/bin/hard", "usr/bin/a\\b")?;
            builder.finish()?;
        }
        let index = index_tarball("data.tar", &tar_bytes)?;
        let mut entries = Vec::new();
        for (path, file) in &index.files {
            entries.push((path.clone(), file.bytes.clone()));
        }
        assert_eq!(
            entries,
            vec![(PathBuf::from("usr/bin/a\\b"), b"data".to_vec())]
        );
        let [
            (
                path,
                TarMember {
                    kind: MemberKind::Hardlink(target),
                    ..
                },
            ),
        ] = index.links_and_dirs.as_slice()
        else {
            panic!("expected one hardlink");
        };
        assert_eq!(
            (path.as_path(), target.as_path()),
            (Path::new("usr/bin/hard"), Path::new("usr/bin/a\\b"))
        );
        Ok(())
    }

    #[test]
    fn file_tree_keeps_header_permission_bits() -> anyhow::Result<()> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let mut dir = tar::Header::new_gnu();
            dir.set_entry_type(tar::EntryType::Directory);
            dir.set_size(0);
            dir.set_mode(0o1777);
            builder.append_data(&mut dir, "./var/tmp/", std::io::empty())?;
            let mut file = tar::Header::new_gnu();
            file.set_size(4);
            file.set_mode(0o4754);
            builder.append_data(&mut file, "./usr/bin/suid", &b"data"[..])?;
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Link);
            link.set_size(0);
            link.set_mode(0o750);
            builder.append_link(&mut link, "./usr/bin/hard", "./usr/bin/suid")?;
            let mut symlink = tar::Header::new_gnu();
            symlink.set_entry_type(tar::EntryType::Symlink);
            symlink.set_size(0);
            symlink.set_mode(0o777);
            builder.append_link(&mut symlink, "./usr/bin/soft", "suid")?;
            builder.finish()?;
        }
        let tree = file_tree(&index_tarball("data.tar", &tar_bytes)?);
        let mode = |name: &str| tree.get(name).map(|entry| entry.mode());
        assert_eq!(mode("var/tmp"), Some(0o1777));
        assert_eq!(mode("usr/bin/suid"), Some(0o4754));
        assert_eq!(mode("usr/bin/hard"), Some(0o750));
        assert_eq!(mode("usr/bin/soft"), Some(0o777));
        assert!(
            tree.get("usr/bin/hard")
                .is_some_and(|entry| entry.is_executable_file())
        );
        assert!(
            !tree
                .get("var/tmp")
                .is_some_and(|entry| entry.is_executable_file())
        );
        let hard = tree.get("usr/bin/hard").expect("usr/bin/hard");
        assert_eq!(hard.hardlink_target(), Some(Path::new("usr/bin/suid")));
        assert_eq!(tree.read(hard)?, b"data");
        assert!(
            tree.get("usr/bin/soft")
                .is_some_and(|entry| entry.hardlink_target().is_none())
        );
        Ok(())
    }

    #[test]
    fn control_file_tree_keeps_header_permission_bits() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb_with_files(
            &path,
            &[("control", b"Package: example\n")],
            &[("usr/bin/app", b"data")],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let control = reader.control_file_tree()?;
        assert_eq!(
            control.get("control").map(|entry| entry.mode()),
            Some(0o644)
        );
        let data = reader.data_file_tree()?;
        assert_eq!(
            data.get("usr/bin/app").map(|entry| entry.mode()),
            Some(0o644)
        );
        Ok(())
    }

    #[test]
    fn member_names_keep_bytes_that_are_not_utf8() -> anyhow::Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let latin1 = b"./usr/share/doc/p/bokm\xe5l";
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let mut file = tar::Header::new_gnu();
            file.set_size(4);
            file.set_mode(0o644);
            builder.append_data(&mut file, OsStr::from_bytes(latin1), &b"data"[..])?;
            let mut dir = tar::Header::new_gnu();
            dir.set_entry_type(tar::EntryType::Directory);
            dir.set_size(0);
            dir.set_mode(0o755);
            builder.append_data(
                &mut dir,
                OsStr::from_bytes(b"./usr/share/\xff/"),
                std::io::empty(),
            )?;
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Link);
            link.set_size(0);
            link.set_mode(0o644);
            builder.append_link(
                &mut link,
                "./usr/share/doc/p/link",
                OsStr::from_bytes(latin1),
            )?;
            builder.finish()?;
        }
        let tree = file_tree(&index_tarball("data.tar", &tar_bytes)?);
        let name = Path::new(OsStr::from_bytes(b"usr/share/doc/p/bokm\xe5l"));
        let file = tree.get(name).expect("raw-byte file name");
        assert!(file.path().to_str().is_none());
        assert_eq!(tree.read(file)?, b"data");
        assert!(tree.get(OsStr::from_bytes(b"usr/share/\xff")).is_some());
        let link = tree.get("usr/share/doc/p/link").expect("hardlink");
        assert_eq!(link.hardlink_target(), Some(name));
        assert_eq!(tree.read(link)?, b"data");
        // The lossy spelling is not the same name.
        assert!(tree.get("usr/share/doc/p/bokm\u{fffd}l").is_none());
        Ok(())
    }

    #[test]
    fn data_file_tree_classifies_elf() -> anyhow::Result<()> {
        use crate::lint::file_tree::EntryKind;
        use crate::lint::file_type::FileType;

        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        let elf = std::fs::read(std::env::current_exe()?)?;
        write_test_deb_with_files(
            &path,
            &[("control", b"Package: example\n")],
            &[("usr/bin/app", elf.as_slice())],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let tree = reader.data_file_tree()?;
        let entry = tree.get("usr/bin/app").expect("usr/bin/app");
        assert_eq!(entry.kind(), EntryKind::RegularFile);
        assert!(
            matches!(
                entry.file_type(),
                Some(FileType::Elf {
                    has_static_symbol_table: true,
                    executable: true
                })
            ),
            "{:?}",
            entry.file_type()
        );
        Ok(())
    }

    #[test]
    fn control_file_tree_lists_control_members() -> anyhow::Result<()> {
        use crate::lint::file_tree::EntryKind;

        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb_with_files(
            &path,
            &[
                ("control", b"Package: example\n"),
                ("./postinst", b"#!/bin/sh\n"),
            ],
            &[("usr/bin/app", b"data")],
        )?;
        let tree = BinaryPackageReader::from_path(&path)?.control_file_tree()?;
        assert_eq!(
            tree.entries()
                .iter()
                .map(|entry| entry.path().to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["control", "postinst"]
        );
        let script = tree.get("postinst").expect("postinst");
        assert_eq!(script.kind(), EntryKind::RegularFile);
        assert_eq!(tree.read(script)?, b"#!/bin/sh\n");
        Ok(())
    }

    #[test]
    fn data_file_tree_lists_directories_and_symlinks() -> anyhow::Result<()> {
        use crate::lint::file_tree::EntryKind;

        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_deb_with_special_data_members(&path)?;
        let tree = BinaryPackageReader::from_path(&path)?.data_file_tree()?;
        let usr = tree.get("usr").expect("usr");
        assert_eq!(usr.kind(), EntryKind::Directory);
        assert!(usr.link_target().is_none());
        let link = tree.get("usr/bin/link").expect("usr/bin/link");
        assert_eq!(link.kind(), EntryKind::Symlink);
        assert_eq!(link.link_target(), Some(Path::new("foo")));
        Ok(())
    }

    #[test]
    fn data_file_tree_keeps_owners_from_tar_headers() -> anyhow::Result<()> {
        use crate::lint::file_tree::Entry;

        let dir = TempDir::new()?;
        let path = dir.path().join("owners.deb");
        write_deb_with_data_members(
            &path,
            &[
                DataMember::file("usr/games/named", 0o2755).owner(0, 60, "root", "games"),
                DataMember::file("usr/bin/numeric", 0o644).owner_ids(1234, 5678),
                DataMember::file("usr/bin/root-numeric", 0o644).owner_ids(0, 0),
                DataMember::file("usr/bin/default", 0o644),
                DataMember::typed("usr/share/dir", tar::EntryType::Directory, 0o755)
                    .owner(0, 50, "root", "staff"),
                DataMember::typed("usr/share/pipe", tar::EntryType::Fifo, 0o600)
                    .owner(0, 50, "root", "staff"),
            ],
        )?;
        let tree = BinaryPackageReader::from_path(&path)?.data_file_tree()?;
        let identity = |name: &str| {
            tree.get(name)
                .and_then(Entry::owner)
                .map(|owner| (owner.identity(), owner.uid(), owner.gid()))
        };
        assert_eq!(
            identity("usr/games/named"),
            Some(("root/games".to_string(), 0, 60))
        );
        // No names in the header: the numbers, except that 0 reads as root.
        assert_eq!(
            identity("usr/bin/numeric"),
            Some(("1234/5678".to_string(), 1234, 5678))
        );
        assert_eq!(
            identity("usr/bin/root-numeric"),
            Some(("root/root".to_string(), 0, 0))
        );
        assert_eq!(
            identity("usr/bin/default"),
            Some(("root/root".to_string(), 0, 0))
        );
        assert_eq!(
            identity("usr/share/dir"),
            Some(("root/staff".to_string(), 0, 50))
        );
        assert_eq!(
            identity("usr/share/pipe"),
            Some(("root/staff".to_string(), 0, 50))
        );
        Ok(())
    }

    #[test]
    fn data_file_tree_lists_fifos_and_devices() -> anyhow::Result<()> {
        use crate::lint::file_tree::{Entry, EntryKind};

        let dir = TempDir::new()?;
        let path = dir.path().join("special.deb");
        write_deb_with_data_members(
            &path,
            &[
                DataMember::typed("etc/init.d/pipe", tar::EntryType::Fifo, 0o755),
                DataMember::typed("dev/tty0", tar::EntryType::Char, 0o620),
                DataMember::typed("dev/sda", tar::EntryType::Block, 0o660),
                DataMember::file("etc/init.d/script", 0o755),
            ],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let tree = reader.data_file_tree()?;
        let kind = |name: &str| tree.get(name).map(Entry::kind);
        assert_eq!(kind("etc/init.d/pipe"), Some(EntryKind::Fifo));
        assert_eq!(kind("dev/tty0"), Some(EntryKind::CharDevice));
        assert_eq!(kind("dev/sda"), Some(EntryKind::BlockDevice));
        assert_eq!(kind("etc/init.d/script"), Some(EntryKind::RegularFile));
        assert_eq!(tree.get("dev/tty0").map(Entry::mode), Some(0o620));
        // Not files: nothing to read, no File type, no link target.
        let pipe = tree.get("etc/init.d/pipe").expect("pipe");
        assert!(tree.read(pipe).is_err());
        assert!(!pipe.is_file());
        assert!(pipe.file_type().is_none());
        assert!(pipe.link_target().is_none());
        assert!(reader.read_data_file("etc/init.d/pipe")?.is_none());
        assert!(
            reader
                .list_data_files()?
                .contains(&PathBuf::from("etc/init.d/pipe"))
        );
        Ok(())
    }

    /// Header ids and, optionally, names.
    type TestOwner = (u64, u64, Option<(&'static str, &'static str)>);

    /// One Data tar member for [`write_deb_with_data_members`].
    struct DataMember {
        path: &'static str,
        kind: tar::EntryType,
        mode: u32,
        owner: Option<TestOwner>,
    }

    impl DataMember {
        fn file(path: &'static str, mode: u32) -> Self {
            Self::typed(path, tar::EntryType::Regular, mode)
        }

        fn typed(path: &'static str, kind: tar::EntryType, mode: u32) -> Self {
            Self {
                path,
                kind,
                mode,
                owner: None,
            }
        }

        fn owner(mut self, uid: u64, gid: u64, user: &'static str, group: &'static str) -> Self {
            self.owner = Some((uid, gid, Some((user, group))));
            self
        }

        /// Ids without names, as an archive written with `--numeric-owner`.
        fn owner_ids(mut self, uid: u64, gid: u64) -> Self {
            self.owner = Some((uid, gid, None));
            self
        }
    }

    fn write_deb_with_data_members(path: &Path, members: &[DataMember]) -> anyhow::Result<()> {
        use std::io::Write;

        use flate2::Compression;
        use flate2::write::GzEncoder;

        let gzip = |tar_bytes: &[u8]| -> anyhow::Result<Vec<u8>> {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(tar_bytes)?;
            Ok(encoder.finish()?)
        };
        let mut control_tar = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut control_tar);
            let mut header = tar::Header::new_gnu();
            header.set_path("control")?;
            header.set_size(8);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &b"Package:"[..])?;
            builder.finish()?;
        }
        let mut data_tar = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut data_tar);
            for member in members {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(member.kind);
                header.set_path(member.path)?;
                header.set_size(0);
                header.set_mode(member.mode);
                if let Some((uid, gid, names)) = member.owner {
                    header.set_uid(uid);
                    header.set_gid(gid);
                    if let Some((user, group)) = names {
                        header.set_username(user)?;
                        header.set_groupname(group)?;
                    }
                }
                header.set_cksum();
                builder.append(&header, std::io::empty())?;
            }
            builder.finish()?;
        }
        write_ar_members(
            path,
            &[
                ("debian-binary", b"2.0\n".as_slice()),
                ("control.tar.gz", &gzip(&control_tar)?),
                ("data.tar.gz", &gzip(&data_tar)?),
            ],
        )
    }

    fn gzip_empty_tar() -> anyhow::Result<Vec<u8>> {
        use std::io::Write;

        use flate2::Compression;
        use flate2::write::GzEncoder;

        let mut tar_bytes = Vec::new();
        tar::Builder::new(&mut tar_bytes).finish()?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_bytes)?;
        Ok(encoder.finish()?)
    }

    fn write_ar_members(path: &Path, members: &[(&str, &[u8])]) -> anyhow::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut builder = ar::Builder::new(file);
        for (name, bytes) in members {
            let header = ar::Header::new(name.as_bytes().to_vec(), bytes.len() as u64);
            builder.append(&header, &mut Cursor::new(*bytes))?;
        }
        Ok(())
    }

    fn write_deb_with_special_data_members(path: &Path) -> anyhow::Result<()> {
        use std::io::Write;

        use flate2::Compression;
        use flate2::write::GzEncoder;

        let control_tar_gz = {
            let mut tar_bytes = Vec::new();
            {
                let mut builder = tar::Builder::new(&mut tar_bytes);
                let mut header = tar::Header::new_gnu();
                header.set_path("control")?;
                header.set_size(8);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, &b"Package:"[..])?;
                builder.finish()?;
            }
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&tar_bytes)?;
            encoder.finish()?
        };

        let data_tar_gz = {
            let mut tar_bytes = Vec::new();
            {
                let mut builder = tar::Builder::new(&mut tar_bytes);
                let mut dir = tar::Header::new_gnu();
                dir.set_entry_type(tar::EntryType::Directory);
                dir.set_path("usr")?;
                dir.set_size(0);
                dir.set_mode(0o755);
                dir.set_cksum();
                builder.append(&dir, std::io::empty())?;

                let mut link = tar::Header::new_gnu();
                link.set_entry_type(tar::EntryType::Symlink);
                link.set_path("usr/bin/link")?;
                link.set_link_name("foo")?;
                link.set_size(0);
                link.set_mode(0o777);
                link.set_cksum();
                builder.append(&link, std::io::empty())?;
                builder.finish()?;
            }
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&tar_bytes)?;
            encoder.finish()?
        };

        write_ar_members(
            path,
            &[
                ("debian-binary", b"2.0\n".as_slice()),
                ("control.tar.gz", &control_tar_gz),
                ("data.tar.gz", &data_tar_gz),
            ],
        )
    }
}

/// A gzipped tar whose names are written as given. `tar::Header::set_path`
/// refuses absolute names, so the name bytes go into the header directly.
#[cfg(test)]
fn gzip_raw_tar(members: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for (name, contents) in members {
            let mut header = tar::Header::new_gnu();
            let name = name.as_bytes();
            header.as_old_mut().name[..name.len()].copy_from_slice(name);
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, *contents)?;
        }
        builder.finish()?;
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&tar_bytes)?;
    Ok(encoder.finish()?)
}

/// A Binary package whose tarball member names are written as given, absolute
/// ones included (`write_test_deb_with_files` cannot make them).
#[cfg(test)]
pub(crate) fn write_test_deb_with_raw_names(
    path: &Path,
    control: &[(&str, &[u8])],
    data: &[(&str, &[u8])],
) -> anyhow::Result<()> {
    let mut builder = ar::Builder::new(std::fs::File::create(path)?);
    let members: [(&[u8], Vec<u8>); 3] = [
        (b"debian-binary", b"2.0\n".to_vec()),
        (b"control.tar.gz", gzip_raw_tar(control)?),
        (b"data.tar.gz", gzip_raw_tar(data)?),
    ];
    for (name, bytes) in &members {
        let header = ar::Header::new(name.to_vec(), bytes.len() as u64);
        builder.append(&header, &mut Cursor::new(bytes))?;
    }
    Ok(())
}

#[cfg(test)]
mod unpack_message_tests {
    use super::*;
    use tempfile::TempDir;

    fn leading_slash() -> Vec<String> {
        vec![LEADING_SLASH_WARNING.to_string()]
    }

    #[test]
    fn leading_slash_members_warn_once_per_tarball() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("pkg.deb");
        write_test_deb_with_raw_names(
            &path,
            &[("control", b"Package: pkg\n"), ("/control-abs", b"x")],
            &[
                ("/usr/bin/a", b"a"),
                ("/usr/bin/b", b"b"),
                ("./usr/share/c", b"c"),
            ],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert_eq!(reader.unpack_messages(Tarball::Data), leading_slash());
        assert_eq!(reader.unpack_messages(Tarball::Control), leading_slash());
        Ok(())
    }

    #[test]
    fn relative_and_dot_members_report_nothing() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("pkg.deb");
        write_test_deb_with_raw_names(
            &path,
            &[("control", b"Package: pkg\n")],
            &[("./usr/bin/a", b"a"), ("usr/bin/b", b"b")],
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert!(reader.unpack_messages(Tarball::Data).is_empty());
        assert!(reader.unpack_messages(Tarball::Control).is_empty());
        Ok(())
    }

    #[test]
    fn a_missing_tarball_has_no_messages() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("pkg.deb");
        let mut builder = ar::Builder::new(std::fs::File::create(&path)?);
        let header = ar::Header::new(b"debian-binary".to_vec(), 4);
        builder.append(&header, &mut Cursor::new(&b"2.0\n"[..]))?;
        let reader = BinaryPackageReader::from_path(&path)?;
        assert!(reader.unpack_messages(Tarball::Data).is_empty());
        assert!(reader.unpack_messages(Tarball::Control).is_empty());
        Ok(())
    }
}
