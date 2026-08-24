use std::path::Path;

use debian_control::lossless::Control;

/// Parsed `debian/control`: first paragraph is source; later named paragraphs are
/// installables. Matches lintian's `Lintian::Debian::Control`, not
/// `debian_control::Control::source()` (which only finds a `Source:` field).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebianControl {
    source: ControlParagraph,
    installables: Vec<Installable>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlParagraph {
    fields: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallablePackageType {
    Deb,
    Udeb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installable {
    name: String,
    package_type: InstallablePackageType,
    fields: ControlParagraph,
}

impl DebianControl {
    /// `None` on syntax errors (lintian leaves required-field to
    /// `syntax-error-in-control-file`). Missing files load as an empty source
    /// paragraph.
    pub fn load(path: &Path) -> Option<Self> {
        if !path.exists() {
            return Some(Self::default());
        }
        match Control::from_file(path) {
            Ok(control) => Some(Self::from_control(&control)),
            Err(_) => None,
        }
    }

    fn from_control(control: &Control) -> Self {
        let mut paragraphs = control.as_deb822().paragraphs();
        let source = paragraphs
            .next()
            .map(|paragraph| ControlParagraph::from_items(paragraph.items()))
            .unwrap_or_default();

        let installables = paragraphs
            .filter_map(|paragraph| {
                let fields = ControlParagraph::from_items(paragraph.items());
                let name = fields.get("Package")?;
                if !is_installable_package_name(name) {
                    return None;
                }
                let package_type = installable_package_type(&fields);
                Some(Installable {
                    name: name.to_string(),
                    package_type,
                    fields,
                })
            })
            .collect();

        Self {
            source,
            installables,
        }
    }

    pub fn source(&self) -> &ControlParagraph {
        &self.source
    }

    pub fn installables(&self) -> &[Installable] {
        &self.installables
    }
}

impl ControlParagraph {
    fn from_items(items: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            fields: items.into_iter().collect(),
        }
    }

    pub fn declares(&self, name: &str) -> bool {
        self.fields
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl Installable {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn package_type(&self) -> InstallablePackageType {
        self.package_type
    }

    pub fn fields(&self) -> &ControlParagraph {
        &self.fields
    }
}

fn installable_package_type(fields: &ControlParagraph) -> InstallablePackageType {
    let value = fields
        .get("Package-Type")
        .or_else(|| fields.get("XC-Package-Type"))
        .unwrap_or("deb");
    if value.eq_ignore_ascii_case("udeb") {
        InstallablePackageType::Udeb
    } else {
        InstallablePackageType::Deb
    }
}

/// lintian's `$PKGNAME_REGEX`: `[a-z0-9][-+\.a-z0-9]+`
fn is_installable_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    let mut rest_len = 0usize;
    for ch in chars {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.')) {
            return false;
        }
        rest_len += 1;
    }
    rest_len >= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_control(contents: &str) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("control");
        fs::write(&path, contents).expect("write control");
        (dir, path)
    }

    #[test]
    fn missing_file_is_empty_source() {
        let path = std::env::temp_dir().join(format!("debmagic-no-control-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let control = DebianControl::load(&path).expect("missing file loads");
        assert!(!control.source().declares("Source"));
        assert!(control.installables().is_empty());
    }

    #[test]
    fn first_paragraph_is_source_without_source_field() {
        let (_dir, path) = write_control(
            "Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n",
        );
        let control = DebianControl::load(&path).expect("parse");
        assert!(!control.source().declares("Source"));
        assert!(control.source().declares("Maintainer"));
        assert_eq!(control.installables().len(), 1);
        assert_eq!(control.installables()[0].name(), "example");
    }

    #[test]
    fn udeb_package_type_from_xc_field() {
        let (_dir, path) = write_control(
            "Source: example\n\
             \n\
             Package: example-udeb\n\
             XC-Package-Type: udeb\n\
             Architecture: all\n\
             Description: udeb\n",
        );
        let control = DebianControl::load(&path).expect("parse");
        assert_eq!(
            control.installables()[0].package_type(),
            InstallablePackageType::Udeb
        );
    }

    #[test]
    fn invalid_package_name_is_not_an_installable() {
        let (_dir, path) = write_control(
            "Source: example\n\
             \n\
             Package: X\n\
             Architecture: all\n\
             Description: too short and uppercase\n",
        );
        let control = DebianControl::load(&path).expect("parse");
        assert!(control.installables().is_empty());
    }

    #[test]
    fn syntax_error_is_unavailable() {
        let (_dir, path) = write_control("not a deb822 file\n: \n");
        assert!(DebianControl::load(&path).is_none());
    }
}
