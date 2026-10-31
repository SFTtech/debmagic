//! What lintian's `debian/copyright` Check reads from a Binary package's
//! `usr/share/doc/<package>/copyright`.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

static COMMON_LICENSES: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/copyright-file/common-licenses"))
        .map(str::to_string)
        .collect()
});

static COMPRESSED_LICENSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)(usr/share/common-licenses/([^ \t]*?)\.gz)").expect("valid regex")
});
static VERSIONLESS_REF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(usr/share/common-licenses/(L?GPL|GFDL))([^-])").expect("valid regex")
});
static OLD_FSF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)(?:Free\s*Software\s*Foundation.*02139|02111-1307|02110-1301)")
        .expect("valid regex")
});
static LATER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:any|or)\s+later(?:\s+version)?\b").expect("valid regex")
});
static LICENSE_ALIAS_PERL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)License-Alias:\s+Perl").expect("valid regex"));
static LICENSE_PERL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)License:\s+Perl").expect("valid regex"));
static LICENSE_PLUS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)License:\s+(L?GPL|GFDL)-[\d.]+\+").expect("valid regex"));
static VERSION_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bversion\b").expect("valid regex"));

#[derive(Debug, Default)]
pub(super) struct InstalledCopyright {
    pub compressed: Vec<PathBuf>,
    pub symlink: Vec<PathBuf>,
    pub old_directory: usize,
    pub old_fsf: bool,
    pub deprecated_bsd: bool,
    pub compressed_licenses: Vec<String>,
    pub incorrect_directories: Vec<String>,
    pub versionless: Vec<String>,
    pub nonexistent: Vec<String>,
}

pub(super) fn inspect(files: &FileTree, package: &str) -> InstalledCopyright {
    let mut found = InstalledCopyright::default();
    let doc = format!("usr/share/doc/{package}");
    let Some(directory) = files.get(&doc) else {
        return found;
    };
    if directory.kind() != EntryKind::Directory {
        return found;
    }
    let gzip_path = PathBuf::from(format!("{doc}/copyright.gz"));
    let plain_path = PathBuf::from(format!("{doc}/copyright"));
    let gzip = files.get(&gzip_path);
    let plain = files.get(&plain_path);
    if gzip.is_some_and(is_regular) {
        found.compressed.push(gzip_path.clone());
    }
    if plain.is_some_and(|entry| entry.kind() == EntryKind::Symlink) {
        found.symlink.push(plain_path.clone());
    }
    let present = gzip.is_some_and(is_regular)
        || plain.is_some_and(|entry| entry.kind() == EntryKind::Symlink || is_regular(entry));
    if !present {
        return found;
    }
    let Some(contents) = text(files, package) else {
        return found;
    };
    scan_contents(&contents, &mut found);
    found
}

/// UTF-8 text of the installed copyright file, when the Check would read it.
///
/// Same path as [`inspect`]: a directory `usr/share/doc/<package>`, then
/// `copyright.gz` (gunzipped) or `copyright` (a symlink is followed). Invalid
/// UTF-8 is skipped. A symlinked doc directory is not read.
pub(super) fn text(files: &FileTree, package: &str) -> Option<String> {
    let doc = format!("usr/share/doc/{package}");
    let directory = files.get(&doc)?;
    if directory.kind() != EntryKind::Directory {
        return None;
    }
    let gzip_path = PathBuf::from(format!("{doc}/copyright.gz"));
    let plain_path = PathBuf::from(format!("{doc}/copyright"));
    let gzip = files.get(&gzip_path);
    let plain = files.get(&plain_path);
    let present = gzip.is_some_and(is_regular)
        || plain.is_some_and(|entry| entry.kind() == EntryKind::Symlink || is_regular(entry));
    if !present {
        return None;
    }
    let bytes = if gzip.is_some_and(is_regular) {
        files
            .read(gzip.expect("gzip entry"))
            .ok()
            .as_deref()
            .and_then(crate::lint::gzip::gunzip)
    } else if let Some(entry) = plain.filter(|entry| is_regular(entry)) {
        files.read(entry).ok()
    } else if let Some(Entry::Symlink { target, .. }) = plain {
        read_link(files, Path::new(&doc), target)
    } else {
        None
    }?;
    String::from_utf8(bytes).ok()
}

fn scan_contents(contents: &str, found: &mut InstalledCopyright) {
    if let Some(captures) = COMPRESSED_LICENSE.captures(contents) {
        let license = captures.get(2).map(|name| name.as_str()).unwrap_or("");
        if COMMON_LICENSES.iter().any(|known| known == license) {
            found.compressed_licenses.push(captures[1].to_string());
        }
    }
    if let Some(captures) = VERSIONLESS_REF.captures(contents) {
        let reference = captures[1].to_string();
        let separator = captures[3].to_string();
        if separator
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        {
            found.nonexistent.push(format!("{reference}{separator}"));
        } else if !later_version(contents, &captures[2]) && regex_version(contents) {
            found.versionless.push(reference);
        }
    }
    if contents.contains("/usr/share/common-licenses/BSD") {
        found.deprecated_bsd = true;
    }
    if let Some(index) = contents.find("usr/share/common-licences") {
        found
            .incorrect_directories
            .push(contents[index..index + "usr/share/common-licences".len()].to_string());
    }
    if contents.contains("usr/share/doc/copyright") {
        found.old_directory += 1;
    }
    if contents.contains("usr/doc/copyright") {
        found.old_directory += 1;
    }
    if OLD_FSF.is_match(contents) {
        found.old_fsf = true;
    }
}

fn later_version(contents: &str, license: &str) -> bool {
    if LATER.is_match(contents) || contents.contains("as Perl itself") {
        return true;
    }
    if LICENSE_ALIAS_PERL.is_match(contents) || LICENSE_PERL.is_match(contents) {
        return true;
    }
    LICENSE_PLUS.captures_iter(contents).any(|captures| {
        captures
            .get(1)
            .is_some_and(|name| name.as_str().eq_ignore_ascii_case(license))
    })
}

fn regex_version(contents: &str) -> bool {
    VERSION_WORD.is_match(contents)
}

fn is_regular(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

fn read_link(files: &FileTree, directory: &Path, target: &Path) -> Option<Vec<u8>> {
    let resolved = if target.is_absolute() {
        target.strip_prefix("/").unwrap_or(target).to_path_buf()
    } else {
        let mut path = directory.to_path_buf();
        for component in target.components() {
            match component {
                std::path::Component::ParentDir => {
                    path.pop();
                }
                std::path::Component::Normal(name) => path.push(name),
                _ => {}
            }
        }
        path
    };
    let entry = files.get(&resolved)?;
    if !is_regular(entry) {
        return None;
    }
    files.read(entry).ok()
}
