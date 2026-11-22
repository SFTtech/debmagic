//! Files named in the `Files` and `Checksums-*` fields of a `.changes`,
//! `.buildinfo` or `.dsc`: lintian's `files` hash of
//! `Lintian::Processable::Fields::Files`.

use super::debian_control::ControlParagraph;

/// The checksum algorithms lintian reads, spelled as in its hints
/// (`Checksum-Md5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    Md5,
    Sha1,
    Sha256,
}

impl ChecksumAlgorithm {
    /// Lintian's name for the algorithm: `Md5`, `Sha1`, `Sha256`.
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::Md5 => "Md5",
            Self::Sha1 => "Sha1",
            Self::Sha256 => "Sha256",
        }
    }
}

/// One checksum line: the sum and the file size that line states.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Checksum {
    algorithm: ChecksumAlgorithm,
    sum: String,
    size: Option<u64>,
}

#[allow(dead_code)]
impl Checksum {
    pub fn algorithm(&self) -> ChecksumAlgorithm {
        self.algorithm
    }

    pub fn sum(&self) -> &str {
        &self.sum
    }

    /// The size this checksum line states; `None` when it is not a number.
    pub fn size(&self) -> Option<u64> {
        self.size
    }
}

/// One file as the fields describe it. `size`, `section` and `priority` come
/// from the `Files` line, so a file only named in a `Checksums-*` field has
/// none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ListedFile {
    name: String,
    size: Option<u64>,
    section: Option<String>,
    priority: Option<String>,
    checksums: Vec<Checksum>,
}

#[allow(dead_code)]
impl ListedFile {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            size: None,
            section: None,
            priority: None,
            checksums: Vec::new(),
        }
    }

    /// The base name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size from the `Files` line.
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// Archive section from the `Files` line of a `.changes` or `.buildinfo`.
    pub fn section(&self) -> Option<&str> {
        self.section.as_deref()
    }

    /// Priority from the `Files` line of a `.changes` or `.buildinfo`.
    pub fn priority(&self) -> Option<&str> {
        self.priority.as_deref()
    }

    pub fn checksums(&self) -> &[Checksum] {
        &self.checksums
    }

    pub fn checksum(&self, algorithm: ChecksumAlgorithm) -> Option<&Checksum> {
        self.checksums
            .iter()
            .find(|checksum| checksum.algorithm == algorithm)
    }
}

/// The files a control paragraph names, in the order they first appear
/// (`Files`, then `Checksums-Sha1`, then `Checksums-Sha256`). A name with a
/// `/` is dropped, as lintian ignores traversal attempts. A line that does not
/// have the expected number of fields stays readable and is recorded in
/// [`FileList::errors`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileList {
    files: Vec<ListedFile>,
    errors: Vec<String>,
}

impl FileList {
    /// `with_section_and_priority` is for the `Files` lines of a `.changes` or
    /// `.buildinfo` (`md5 size section priority name`); a `.dsc` has
    /// `md5 size name`.
    pub fn parse(fields: &ControlParagraph, with_section_and_priority: bool) -> Self {
        let mut list = Self::default();
        let expected = if with_section_and_priority { 5 } else { 3 };
        for line in lines(fields.get("Files")) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some(name) = words.last().copied() else {
                continue;
            };
            if name.contains('/') {
                continue;
            }
            if words.len() != expected {
                list.errors
                    .push(format!("Files line has {} fields: {line}", words.len()));
            }
            let size = words.get(1).and_then(|size| list.number(size, &line));
            let file = list.file_mut(name);
            file.size = size;
            if with_section_and_priority {
                file.section = words.get(2).map(|section| (*section).to_string());
                file.priority = words.get(3).map(|priority| (*priority).to_string());
            }
            if let Some(sum) = words.first() {
                list.add_checksum(name, ChecksumAlgorithm::Md5, sum, size);
            }
        }
        for (field, algorithm) in [
            ("Checksums-Sha1", ChecksumAlgorithm::Sha1),
            ("Checksums-Sha256", ChecksumAlgorithm::Sha256),
        ] {
            for line in lines(fields.get(field)) {
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.len() != 3 {
                    list.errors
                        .push(format!("{field} line has {} fields: {line}", words.len()));
                    if words.len() < 3 {
                        continue;
                    }
                }
                let (sum, size, name) = (words[0], words[1], words[2]);
                if name.contains('/') {
                    continue;
                }
                let size = list.number(size, &line);
                list.add_checksum(name, algorithm, sum, size);
            }
        }
        list
    }

    fn number(&mut self, text: &str, line: &str) -> Option<u64> {
        let number = text.parse().ok();
        if number.is_none() {
            self.errors
                .push(format!("size '{text}' is not a number: {line}"));
        }
        number
    }

    fn file_mut(&mut self, name: &str) -> &mut ListedFile {
        let index = match self.files.iter().position(|file| file.name == name) {
            Some(index) => index,
            None => {
                self.files.push(ListedFile::new(name));
                self.files.len() - 1
            }
        };
        &mut self.files[index]
    }

    fn add_checksum(
        &mut self,
        name: &str,
        algorithm: ChecksumAlgorithm,
        sum: &str,
        size: Option<u64>,
    ) {
        self.file_mut(name).checksums.push(Checksum {
            algorithm,
            sum: sum.to_string(),
            size,
        });
    }

    #[allow(dead_code)]
    pub fn files(&self) -> &[ListedFile] {
        &self.files
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&ListedFile> {
        self.files.iter().find(|file| file.name == name)
    }

    /// Lines that did not have the expected shape.
    #[allow(dead_code)]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

/// The non-empty, trimmed lines of a multi-line field value.
fn lines(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(fields: &[(&str, &str)]) -> ControlParagraph {
        ControlParagraph::from_fields(fields.iter().map(|(name, value)| (*name, *value)))
    }

    #[test]
    fn changes_files_carry_section_priority_and_checksums() {
        let fields = paragraph(&[
            (
                "Files",
                "\n d41d8cd98f00b204e9800998ecf8427e 12 non-free optional a.deb\n",
            ),
            ("Checksums-Sha1", "\n aaaa 12 a.deb\n bbbb 3 b.deb"),
        ]);
        let list = FileList::parse(&fields, true);
        assert!(list.errors().is_empty(), "{:?}", list.errors());
        assert_eq!(list.files().len(), 2);
        let a = list.get("a.deb").expect("a.deb");
        assert_eq!(a.size(), Some(12));
        assert_eq!(a.section(), Some("non-free"));
        assert_eq!(a.priority(), Some("optional"));
        assert_eq!(
            a.checksum(ChecksumAlgorithm::Md5).map(Checksum::sum),
            Some("d41d8cd98f00b204e9800998ecf8427e")
        );
        assert_eq!(
            a.checksum(ChecksumAlgorithm::Sha1).map(Checksum::sum),
            Some("aaaa")
        );
        assert!(a.checksum(ChecksumAlgorithm::Sha256).is_none());
        // Only in a Checksums field: no size, section or priority.
        let b = list.get("b.deb").expect("b.deb");
        assert_eq!(b.size(), None);
        assert_eq!(b.section(), None);
        assert_eq!(
            b.checksum(ChecksumAlgorithm::Sha1).and_then(Checksum::size),
            Some(3)
        );
    }

    #[test]
    fn dsc_files_have_no_section() {
        let fields = paragraph(&[("Files", "\n abc 5 foo_1.0.tar.xz")]);
        let list = FileList::parse(&fields, false);
        assert!(list.errors().is_empty());
        let file = list.get("foo_1.0.tar.xz").expect("tarball");
        assert_eq!(file.size(), Some(5));
        assert_eq!(file.section(), None);
    }

    #[test]
    fn traversal_names_are_dropped_and_bad_lines_recorded() {
        let fields = paragraph(&[(
            "Files",
            "\n abc 5 a b ../evil.deb\n abc x s p ok.deb\n short",
        )]);
        let list = FileList::parse(&fields, true);
        let names: Vec<&str> = list.files().iter().map(ListedFile::name).collect();
        assert_eq!(names, ["ok.deb", "short"]);
        assert_eq!(list.get("ok.deb").and_then(ListedFile::size), None);
        assert_eq!(list.errors().len(), 2, "{:?}", list.errors());
    }
}
