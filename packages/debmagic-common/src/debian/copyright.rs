/// The `Files-Excluded` patterns from a `debian/copyright` file.
///
/// The field is a machine-readable header (not part of the deb822
/// paragraphs): `Files-Excluded:` for the main tree and
/// `Files-Excluded-<component>:` for a multiple-upstream-tarballs
/// component, each with indented or same-line glob patterns.
#[derive(Debug, Clone, Default)]
pub struct FilesExcluded {
    /// Patterns of the main `Files-Excluded:` field.
    pub main: Vec<String>,
    /// Patterns per component, keyed by component name.
    pub components: std::collections::HashMap<String, Vec<String>>,
}

/// Parse the `Files-Excluded` fields from a `debian/copyright`
/// file's content.
pub fn files_excluded(content: &str) -> FilesExcluded {
    let mut result = FilesExcluded::default();
    // the component whose patterns we are collecting; `None` = main
    let mut current: Option<Option<String>> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(field) = trimmed.strip_prefix("Files-Excluded") {
            // `Files-Excluded:` (main) or `Files-Excluded-<component>:`
            let (component, rest) = match field.strip_prefix(':') {
                Some(rest) => (None, rest),
                None => match field.strip_prefix('-').and_then(|f| f.split_once(':')) {
                    Some((component, rest))
                        if !component.is_empty() && !component.contains(char::is_whitespace) =>
                    {
                        (Some(component.to_string()), rest)
                    }
                    _ => {
                        current = None;
                        continue;
                    }
                },
            };
            current = Some(component.clone());
            let target = match &component {
                Some(component) => result.components.entry(component.clone()).or_default(),
                None => &mut result.main,
            };
            let inline = rest.trim();
            if !inline.is_empty() {
                target.push(inline.to_string());
            }
            continue;
        }
        if trimmed.is_empty() {
            current = None;
            continue;
        }
        if let Some(component) = &current {
            // patterns are indented under the field
            if line.starts_with(' ') || line.starts_with('\t') {
                let target = match component {
                    Some(component) => result.components.entry(component.clone()).or_default(),
                    None => &mut result.main,
                };
                target.push(trimmed.to_string());
            } else {
                current = None;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_excluded_main() {
        let content = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nFiles-Excluded:\n win32/*\n docs/html/*\n\nFiles: *\nCopyright: X\nLicense: MIT\n";
        let excluded = files_excluded(content);
        assert_eq!(excluded.main, vec!["win32/*", "docs/html/*"]);
        assert!(excluded.components.is_empty());
    }

    #[test]
    fn test_files_excluded_inline_and_component() {
        let content =
            "Files-Excluded: win32/*\nFiles-Excluded-doc:\n doc/*\nFiles: *\nCopyright: X\n";
        let excluded = files_excluded(content);
        assert_eq!(excluded.main, vec!["win32/*"]);
        assert_eq!(excluded.components["doc"], vec!["doc/*"]);
    }

    #[test]
    fn test_files_excluded_none() {
        let content = "Files: *\nCopyright: X\nLicense: MIT\n";
        let excluded = files_excluded(content);
        assert!(excluded.main.is_empty());
        assert!(excluded.components.is_empty());
    }
}
