use std::path::Path;

use debmagic_common::debian::deb822::{self, Paragraph};

use super::package_relation::PackageRelation;

pub(crate) use debmagic_common::debian::deb822::unfold;

/// Parsed `debian/control`: first paragraph is source; later named paragraphs are
/// installables. Matches lintian's `Lintian::Debian::Control`, not
/// `debian_control::Control::source()` (which only finds a `Source:` field).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebianControl {
    source: ControlParagraph,
    installables: Vec<Installable>,
}

/// One Deb822 paragraph of a control file, read through the common
/// [`Paragraph`], plus lintian's relation parsing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlParagraph(Paragraph);

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
        let text = std::fs::read_to_string(path).ok()?;
        Self::parse(&text)
    }

    /// `None` on syntax errors. Same first-paragraph-is-source shape as [`Self::load`].
    pub fn parse(text: &str) -> Option<Self> {
        let paragraphs = deb822::parse(text)
            .into_result()
            .or_else(|_| deb822::parse(&whitespace_only_lines_as_blank(text)).into_result())
            .ok()?;
        Some(Self::from_paragraphs(paragraphs))
    }

    fn from_paragraphs(paragraphs: Vec<Paragraph>) -> Self {
        let mut paragraphs = paragraphs.into_iter().map(ControlParagraph);
        let source = paragraphs.next().unwrap_or_default();
        let installables = paragraphs
            .filter_map(|fields| {
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
    #[cfg(test)]
    pub(crate) fn from_fields(
        fields: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self(Paragraph::from_values(fields))
    }

    /// The only paragraph of a control file, or `Err` naming `what` when
    /// the text is not valid Deb822 or not exactly one paragraph.
    pub(crate) fn parse_single(text: &str, what: &str) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let mut paragraphs = deb822::parse(text)
            .into_result()
            .with_context(|| format!("{what} is not valid Deb822"))?
            .into_iter();
        let Some(first) = paragraphs.next() else {
            anyhow::bail!("{what} has no Deb822 paragraph");
        };
        if paragraphs.next().is_some() {
            anyhow::bail!("{what} is not exactly one Deb822 paragraph");
        }
        Ok(Self(first))
    }

    /// The first Deb822 paragraph of a file that may carry a PGP signature
    /// (a `.dsc`, `.changes` or `.buildinfo`). `what` names the file kind in
    /// the error. Not readable, not valid, or without a paragraph is an `Err`.
    pub(crate) fn parse_first_signed(text: &str, what: &str) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let (payload, _) = debian_control::pgp::strip_pgp_signature(text)
            .with_context(|| format!("{what} is not readable Deb822"))?;
        let paragraphs = deb822::parse(&payload)
            .into_result()
            .with_context(|| format!("{what} is not valid Deb822"))?;
        let Some(first) = paragraphs.into_iter().next() else {
            anyhow::bail!("{what} has no Deb822 paragraph");
        };
        Ok(Self(first))
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name)
    }

    pub fn declares(&self, name: &str) -> bool {
        self.0.declares(name)
    }

    /// Field names in file order, as written.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.names()
    }

    /// The value on one line (lintian's `unfolded_value`); see [`unfold`].
    pub fn unfolded_value(&self, name: &str) -> Option<String> {
        self.0.unfolded_value(name)
    }

    /// lintian's `untrimmed_value`; see [`Paragraph::untrimmed_value`].
    pub fn untrimmed_value(&self, name: &str) -> Option<&str> {
        self.0.untrimmed_value(name)
    }

    /// The 1-based line where the field's name is written; see
    /// [`Paragraph::position`].
    pub fn position(&self, name: &str) -> Option<u32> {
        self.0.position(name)
    }

    /// A relation field (`Depends`, `Build-Depends`, ...) parsed per Policy
    /// 7.1. `None` when the paragraph does not declare it. Malformed text
    /// stays on the parsed alternatives.
    pub(crate) fn relation(&self, name: &str) -> Option<PackageRelation> {
        self.unfolded_value(name)
            .map(|value| PackageRelation::parse(&value))
    }

    /// The named fields AND-ed, in order. A field that is not declared is
    /// skipped. This is lintian's `Build-Depends-All` and binary `all`.
    pub(crate) fn relations_and(&self, names: &[&str]) -> PackageRelation {
        names
            .iter()
            .fold(PackageRelation::default(), |combined, name| {
                self.relation(name)
                    .map(|relation| combined.and(&relation))
                    .unwrap_or(combined)
            })
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

/// A whitespace-only line ends a paragraph for lintian (`Lintian::Deb822`
/// treats `/^\s*$/` as a blank line), so a file with one just before the next
/// stanza is fine for lintian. `deb822-lossless` rejects that line as an
/// "empty continuation line" and the whole file would be unavailable. Rewrites
/// such lines to empty ones, the same stanza split lintian makes. Line count
/// is unchanged, so reported lines stay correct. Used only as the retry after
/// a failed parse.
fn whitespace_only_lines_as_blank(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_paragraph = false;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let ending = &line[body.len()..];
        if in_paragraph && !body.is_empty() && body.chars().all(|ch| ch == ' ' || ch == '\t') {
            out.push_str(ending);
        } else {
            out.push_str(line);
        }
        in_paragraph = !body.is_empty() && !body.chars().all(|ch| ch == ' ' || ch == '\t');
    }
    out
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
        assert!(DebianControl::parse("not a deb822 file\n: \n").is_none());
    }

    #[test]
    fn whitespace_only_line_ends_stanza_like_lintian() {
        // libcddb's debian/control ends the libcddb2-dev stanza with a line
        // holding one space, directly followed by the next stanza. lintian
        // reads that as a paragraph end; the lossless parser alone does not.
        let text = "Source: libcddb\n\
Build-Depends: debhelper (>= 9),\n  debhelper-compat (= 13)\n\
\n\
Package: libcddb2-dev\n\
Architecture: any\n\
Description: development files\n headers.\n\x20\n\
Package: libcddb2\n\
Architecture: any\n\
Description: runtime\n runtime.\n";
        let parsed = DebianControl::parse(text).expect("blank continuation parses");
        assert_eq!(
            parsed.source().get("Build-Depends"),
            Some("debhelper (>= 9),\ndebhelper-compat (= 13)")
        );
        assert_eq!(parsed.installables().len(), 2);
        assert_eq!(parsed.installables()[0].name(), "libcddb2-dev");
        assert_eq!(parsed.installables()[1].name(), "libcddb2");
    }

    #[test]
    fn parse_matches_load() {
        let text = "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n";
        let parsed = DebianControl::parse(text).expect("parse");
        assert_eq!(parsed.source().get("Source"), Some("example"));
        assert_eq!(parsed.installables().len(), 1);
    }

    #[test]
    fn position_is_the_line_of_each_field_name() {
        let text = "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Vcs-Git: https://example.org/repo.git\n\
             \n\
             # comment before the stanza\n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n\
             Priority: optional\n";
        let parsed = DebianControl::parse(text).expect("parse");
        assert_eq!(parsed.source().position("Source"), Some(1));
        assert_eq!(parsed.source().position("vcs-git"), Some(3));
        assert_eq!(parsed.source().position("Missing"), None);
        let installable = &parsed.installables()[0];
        assert_eq!(installable.fields().position("Package"), Some(6));
        assert_eq!(installable.fields().position("Description"), Some(8));
        assert_eq!(installable.fields().position("Priority"), Some(10));
    }

    #[test]
    fn position_is_none_for_plain_values() {
        let paragraph = ControlParagraph::from_fields([("Source", "example")]);
        assert_eq!(paragraph.position("Source"), None);
        assert_eq!(paragraph.get("Source"), Some("example"));
    }
}
