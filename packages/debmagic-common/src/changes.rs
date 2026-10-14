use deb822_lossless::Paragraph;

use crate::artifact::is_orig_tarball;
use crate::checksums::Hash;

/// A parsed `.changes` file listing: which files it references and
/// whether the `orig` tarball(s) are among them.
#[derive(Debug, Clone, Default)]
pub struct ChangesFiles {
    /// All filenames listed under `Files:`/`Checksums-*:`.
    pub files: Vec<String>,
}

impl ChangesFiles {
    /// The `orig` tarball entries: `<source>_<version>.orig.tar.<ext>`
    /// and component origs `<source>_<version>.orig-<component>.tar.<ext>`.
    pub fn orig_tarballs(&self) -> Vec<&str> {
        self.files
            .iter()
            .map(String::as_str)
            .filter(|name| is_orig_tarball(name))
            .collect()
    }
}

/// Add one `orig` tarball entry (or, with `None`, remove all orig
/// entries) from a `.changes` paragraph's `Files:`/`Checksums-*:`
/// listings. `tarball_data` is the content of the tarball named by
/// `orig_name`, used for the checksum and size entries; it is unused
/// when removing. Returns whether the paragraph was modified.
pub fn set_changes_orig_entries(
    control: &mut Paragraph,
    orig_name: Option<&str>,
    tarball_data: &[u8],
) -> bool {
    let mut modified = false;

    let keys: Vec<String> = control.keys().collect();
    for key in keys {
        if key != "Files" && !key.starts_with("Checksums-") {
            continue;
        }
        let Some(value) = control.get(&key) else {
            continue;
        };
        let mut lines: Vec<String> = Vec::new();
        for line in value.lines().filter(|l| !l.is_empty()) {
            let name = line.split_whitespace().next_back().unwrap_or_default();
            let is_orig = is_orig_tarball(name);
            match orig_name {
                // adding: keep every existing entry, origs included
                Some(_) => lines.push(line.to_string()),
                // removing: drop all orig entries
                None if is_orig => modified = true,
                None => lines.push(line.to_string()),
            }
        }
        if let Some(name) = orig_name {
            let already = lines
                .iter()
                .any(|l| l.split_whitespace().next_back() == Some(name));
            if !already {
                modified = true;
                let hash = match key.as_str() {
                    "Files" => Hash::Md5.hex(tarball_data),
                    "Checksums-Sha1" => Hash::Sha1.hex(tarball_data),
                    "Checksums-Sha256" => Hash::Sha256.hex(tarball_data),
                    _ => continue,
                };
                let mut entry = format!("{hash} {}", tarball_data.len());
                // `.changes` Files entries carry `section priority` between
                // size and name; every source file shares the source's, so
                // copy them from an existing entry
                if key == "Files"
                    && let Some(line) = lines.first()
                    && let [_, _, section, priority, _] =
                        line.split_whitespace().collect::<Vec<_>>()[..]
                {
                    entry = format!("{entry} {section} {priority}");
                }
                lines.push(format!("{entry} {name}"));
            }
        }
        control.set(&key, &lines.join("\n"));
    }

    modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changes_files_orig_tarballs() {
        let changes = ChangesFiles {
            files: vec![
                "pkg_1.0.orig.tar.xz".to_string(),
                "pkg_1.0.orig-bar.tar.xz".to_string(),
                "pkg_1.0-1.debian.tar.xz".to_string(),
                "pkg_1.0-1.dsc".to_string(),
            ],
        };
        assert_eq!(
            changes.orig_tarballs(),
            vec!["pkg_1.0.orig.tar.xz", "pkg_1.0.orig-bar.tar.xz"]
        );
    }

    #[test]
    fn set_changes_orig_entries_adds_and_removes() {
        let mut control: Paragraph = "Format: 1.8\nSource: pkg\nFiles:\n abc 3 devel optional pkg_1.0-1.debian.tar.xz\n def 4 devel optional pkg_1.0-1.dsc\nChecksums-Sha256:\n xyz 3 pkg_1.0-1.debian.tar.xz\n zzz 4 pkg_1.0-1.dsc\n"
            .parse()
            .unwrap();

        // add: entries match the field's format and indentation
        assert!(set_changes_orig_entries(
            &mut control,
            Some("pkg_1.0.orig.tar.xz"),
            b"tarball"
        ));
        let content = control.to_string();
        assert!(content.contains(&format!(
            "\n {} 7 devel optional pkg_1.0.orig.tar.xz\n",
            Hash::Md5.hex(b"tarball")
        )));
        assert!(content.contains(&format!(
            "\n {} 7 pkg_1.0.orig.tar.xz\n",
            Hash::Sha256.hex(b"tarball")
        )));

        // adding again is a no-op
        assert!(!set_changes_orig_entries(
            &mut control,
            Some("pkg_1.0.orig.tar.xz"),
            b"tarball"
        ));

        // adding a component orig keeps the main one
        assert!(set_changes_orig_entries(
            &mut control,
            Some("pkg_1.0.orig-bar.tar.xz"),
            b"component"
        ));
        let content = control.to_string();
        assert!(content.contains("pkg_1.0.orig.tar.xz"));
        assert!(content.contains("pkg_1.0.orig-bar.tar.xz"));

        // remove drops all origs, main and component
        assert!(set_changes_orig_entries(&mut control, None, b""));
        let content = control.to_string();
        assert!(!content.contains("orig.tar"));
        assert!(!content.contains("orig-bar"));
    }
}
