//! Members of an `ar` archive (a static library, `.a`), read from its bytes:
//! the GNU long-name table and `/N` names, the BSD `#1/N` names, and the
//! archive symbol table, which `ar t` does not list.
//!
//! This is a small parser of the common formats, not a full `ar` reader: a
//! truncated header ends the list, and the member data is clipped to the
//! archive.

/// One member of an archive: its name as `ar t` prints it, and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member<'a> {
    pub name: String,
    pub data: &'a [u8],
}

/// Whether `bytes` start with the `ar` magic (`file(1)`: "current ar
/// archive").
pub(crate) fn is_archive(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

const MAGIC: &[u8] = b"!<arch>\n";

/// The members of an `ar` archive in archive order. Empty when the bytes are
/// not an archive.
pub(crate) fn members(bytes: &[u8]) -> Vec<Member<'_>> {
    let mut members = Vec::new();
    if !is_archive(bytes) {
        return members;
    }
    let mut long_names: &[u8] = b"";
    let mut position = MAGIC.len();
    while position + 60 <= bytes.len() {
        let header = &bytes[position..position + 60];
        let raw_name = &header[..16];
        let size: usize = match std::str::from_utf8(&header[48..58])
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            Some(size) => size,
            None => break,
        };
        let data_start = position + 60;
        let data_end = data_start.saturating_add(size).min(bytes.len());
        let data = &bytes[data_start..data_end];

        if raw_name.starts_with(b"//") {
            // The long-name table.
            long_names = data;
        } else if is_gnu_long_name(raw_name) {
            // A GNU long name: an offset into the table, ended by `/\n`.
            let offset: usize = std::str::from_utf8(&raw_name[1..])
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(usize::MAX);
            let table = long_names.get(offset..).unwrap_or(&[]);
            let end = table
                .windows(2)
                .position(|pair| pair == b"/\n")
                .unwrap_or(table.len());
            members.push(Member {
                name: String::from_utf8_lossy(&table[..end]).into_owned(),
                data,
            });
        } else if raw_name.starts_with(b"#1/") {
            // A BSD long name: the first bytes of the data, which the member
            // data follows.
            let length: usize = std::str::from_utf8(&raw_name[3..])
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(0);
            let name = &data[..length.min(data.len())];
            members.push(Member {
                name: String::from_utf8_lossy(name).into_owned(),
                data: &data[length.min(data.len())..],
            });
        } else if raw_name.starts_with(b"/ ") {
            // The archive symbol table, which `ar t` does not list.
        } else {
            let name = raw_name
                .split(|&byte| byte == b'/')
                .next()
                .unwrap_or_default();
            members.push(Member {
                name: String::from_utf8_lossy(name).trim_end().to_string(),
                data,
            });
        }
        position = data_start + size + (size % 2);
    }
    members
}

/// `/N` followed by the space padding of the name field.
fn is_gnu_long_name(raw_name: &[u8]) -> bool {
    let Some(digits) = raw_name.strip_prefix(b"/") else {
        return false;
    };
    let digits = digits.trim_ascii_end();
    !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header for `name` with `size` bytes of data, padded to 60 bytes.
    fn header(name: &str, size: usize) -> Vec<u8> {
        format!(
            "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            0, 0, 0, 644, size
        )
        .into_bytes()
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        for (name, data) in entries {
            bytes.extend(header(name, data.len()));
            bytes.extend_from_slice(data);
            if data.len() % 2 == 1 {
                bytes.push(b'\n');
            }
        }
        bytes
    }

    #[test]
    fn plain_and_gnu_names_with_their_data() {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(header("//", 13));
        bytes.extend_from_slice(b"very-long.o/\n");
        bytes.push(b'\n'); // padding to an even size
        bytes.extend(header("short.o/", 3));
        bytes.extend_from_slice(b"abc\n");
        bytes.extend(header("/0", 2));
        bytes.extend_from_slice(b"xy");
        let members = members(&bytes);
        assert_eq!(
            members,
            [
                Member {
                    name: "short.o".to_string(),
                    data: b"abc",
                },
                Member {
                    name: "very-long.o".to_string(),
                    data: b"xy",
                },
            ]
        );
    }

    #[test]
    fn bsd_names_precede_their_data() {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(header("#1/10", 10));
        bytes.extend_from_slice(b"bsd-name.o");
        let members = members(&bytes);
        assert_eq!(
            members,
            [Member {
                name: "bsd-name.o".to_string(),
                data: b"",
            }]
        );
    }

    #[test]
    fn symbol_table_is_not_a_member() {
        let bytes = archive(&[("/", b"\0\0\0\0"), ("a.o/", b"x")]);
        let names: Vec<String> = members(&bytes).into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["a.o"]);
    }

    #[test]
    fn not_an_archive_has_no_members() {
        assert!(!is_archive(b"hello"));
        assert!(members(b"hello").is_empty());
        assert!(members(b"!<arch>\n").is_empty());
    }
}
