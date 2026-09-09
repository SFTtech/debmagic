use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// In-process reader for a Binary package. Callers list and read Control files
/// and Data files; they do not hold the inner tarballs.
pub(crate) struct BinaryPackageReader {
    path: PathBuf,
    control_tar_name: String,
    control_tar: Vec<u8>,
    #[allow(dead_code)]
    data_tar_name: String,
    #[allow(dead_code)]
    data_tar: Vec<u8>,
    control_index: OnceCell<TarballIndex>,
    #[allow(dead_code)]
    data_index: OnceCell<TarballIndex>,
}

struct TarballIndex {
    #[allow(dead_code)]
    members: Vec<String>,
    files: HashMap<String, Vec<u8>>,
}

impl BinaryPackageReader {
    /// Open a Binary package. `debian-binary`, a Control tarball, and a Data
    /// tarball must all be present (deb(5)); their contents are indexed later.
    pub(crate) fn from_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let file = std::fs::File::open(&path)
            .with_context(|| format!("opening {} failed", path.display()))?;
        let mut archive = ar::Archive::new(file);
        let mut debian_binary = false;
        let mut control_tar = None;
        let mut data_tar = None;
        while let Some(entry) = archive.next_entry() {
            let mut entry =
                entry.with_context(|| format!("reading ar member from {}", path.display()))?;
            let name = ar_member_name(entry.header().identifier());
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("reading {name} from {}", path.display()))?;
            if name == "debian-binary" {
                debian_binary = true;
            } else if name.starts_with("control.tar") {
                if control_tar.is_none() {
                    control_tar = Some((name, bytes));
                }
            } else if name.starts_with("data.tar") && data_tar.is_none() {
                data_tar = Some((name, bytes));
            }
        }
        if !debian_binary {
            bail!("{} has no debian-binary member", path.display());
        }
        let Some((control_tar_name, control_tar)) = control_tar else {
            bail!("{} has no control.tar member", path.display());
        };
        let Some((data_tar_name, data_tar)) = data_tar else {
            bail!("{} has no data.tar member", path.display());
        };
        Ok(Self {
            path,
            control_tar_name,
            control_tar,
            data_tar_name,
            data_tar,
            control_index: OnceCell::new(),
            data_index: OnceCell::new(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    #[allow(dead_code)]
    pub(crate) fn list_control_files(&self) -> anyhow::Result<&[String]> {
        Ok(&self.control_index()?.members)
    }

    #[allow(dead_code)]
    pub(crate) fn list_data_files(&self) -> anyhow::Result<&[String]> {
        Ok(&self.data_index()?.members)
    }

    pub(crate) fn read_control_file(&self, path: &str) -> anyhow::Result<Option<&[u8]>> {
        Ok(lookup_file(self.control_index()?, path))
    }

    #[allow(dead_code)]
    pub(crate) fn read_data_file(&self, path: &str) -> anyhow::Result<Option<&[u8]>> {
        Ok(lookup_file(self.data_index()?, path))
    }

    fn control_index(&self) -> anyhow::Result<&TarballIndex> {
        init_index(
            &self.control_index,
            &self.control_tar_name,
            &self.control_tar,
            self.path.as_path(),
        )
    }

    #[allow(dead_code)]
    fn data_index(&self) -> anyhow::Result<&TarballIndex> {
        init_index(
            &self.data_index,
            &self.data_tar_name,
            &self.data_tar,
            self.path.as_path(),
        )
    }
}

fn init_index<'a>(
    cell: &'a OnceCell<TarballIndex>,
    name: &str,
    compressed: &[u8],
    archive_path: &Path,
) -> anyhow::Result<&'a TarballIndex> {
    if cell.get().is_none() {
        let index = index_tarball(name, compressed)
            .with_context(|| format!("indexing {name} from {}", archive_path.display()))?;
        let _ = cell.set(index);
    }
    Ok(cell.get().expect("tarball index is initialized"))
}

fn lookup_file<'a>(index: &'a TarballIndex, path: &str) -> Option<&'a [u8]> {
    let key = normalize_member_path(Path::new(path))?;
    index.files.get(&key).map(Vec::as_slice)
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
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let Some(normalized) = normalize_member_path(path.as_ref()) else {
            continue;
        };
        if seen.insert(normalized.clone()) {
            members.push(normalized.clone());
        }
        if entry.header().entry_type().is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            files.insert(normalized, bytes);
        }
    }
    Ok(TarballIndex { members, files })
}

fn normalize_member_path(path: &Path) -> Option<String> {
    let mut value = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = value.strip_prefix("./") {
        value = stripped.to_string();
    }
    if let Some(stripped) = value.strip_prefix('/') {
        value = stripped.to_string();
    }
    let value = value.trim_end_matches('/');
    if value.is_empty() || value == "." || value == ".." {
        return None;
    }
    Some(value.to_string())
}

fn decompress_tar(name: &str, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(bytes);
    if name.ends_with(".gz") {
        let mut decoder = flate2::read::GzDecoder::new(cursor);
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
    fn missing_data_tarball_is_an_error() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("control-only.deb");
        write_ar_members(
            &path,
            &[
                ("debian-binary", b"2.0\n".as_slice()),
                ("control.tar.gz", &gzip_empty_tar()?),
            ],
        )?;
        let error = BinaryPackageReader::from_path(&path)
            .err()
            .expect("control-only archive");
        assert!(error.to_string().contains("data.tar"), "{error}");
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
            ["control", "md5sums"].as_slice()
        );
        assert_eq!(
            reader.list_data_files()?,
            ["usr/bin/foo", "usr/share/doc/example/copyright"].as_slice()
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
        assert!(listed.contains(&"usr".to_string()), "{listed:?}");
        assert!(listed.contains(&"usr/bin/link".to_string()), "{listed:?}");
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
