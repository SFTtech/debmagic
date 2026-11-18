//! Line numbers of fields in `debian/control`, and build-dependency names.
//!
//! Installable paragraphs are those with a `Package` field. Field position is
//! the physical line the field name is written on. Name presence walks
//! `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` (lintian's
//! `Build-Depends-All`) and compares package names, ignoring versions,
//! architecture qualifiers, and restrictions.

use crate::lint::debian_control::ControlParagraph;

const BUILD_DEPENDS_ALL: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

pub(super) struct InstallableParagraph {
    pub(super) package: String,
    fields: Vec<(String, u32)>,
}

impl InstallableParagraph {
    pub(super) fn field_line(&self, name: &str) -> Option<u32> {
        self.fields
            .iter()
            .find(|(field, _)| field.eq_ignore_ascii_case(name))
            .map(|(_, line)| *line)
    }
}

pub(super) fn installable_paragraphs(text: &str) -> Vec<InstallableParagraph> {
    let mut paragraphs = Vec::new();
    let mut fields = Vec::new();
    let mut package = None;

    let mut flush = |fields: &mut Vec<(String, u32)>, package: &mut Option<String>| {
        if let Some(package) = package.take() {
            paragraphs.push(InstallableParagraph {
                package,
                fields: std::mem::take(fields),
            });
        } else {
            fields.clear();
        }
    };

    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if line.trim().is_empty() {
            if !fields.is_empty() || package.is_some() {
                flush(&mut fields, &mut package);
            }
            continue;
        }
        if line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if name.eq_ignore_ascii_case("Package") {
            let value = value.trim();
            if !value.is_empty() {
                package = Some(value.to_string());
            }
        }
        fields.push((name.to_string(), number));
    }
    if !fields.is_empty() || package.is_some() {
        flush(&mut fields, &mut package);
    }
    paragraphs
}

/// Package names declared by the source paragraph's build-dependency fields.
pub(in super::super) fn build_depends_all_names(source: &ControlParagraph) -> Vec<String> {
    let mut names = Vec::new();
    for field in BUILD_DEPENDS_ALL {
        let Some(relation) = source.relation(field) else {
            continue;
        };
        for alternative in relation.alternatives() {
            if !alternative.package().is_empty() {
                names.push(alternative.package().to_string());
            }
        }
    }
    names
}

pub(in super::super) fn names_package(names: &[String], package: &str) -> bool {
    names.iter().any(|name| name == package)
}

#[cfg(test)]
mod tests {
    use crate::lint::debian_control::DebianControl;

    use super::*;

    #[test]
    fn package_names_and_field_lines_follow_the_file() {
        let text = "\
Source: example\n\
Build-Depends: debhelper\n\
\n\
# comment\n\
Package: foo++\n\
Architecture: all\n\
Build-Depends: test\n\
Description: see\n extra\n";
        let control = DebianControl::parse(text).expect("control");
        let paragraphs = installable_paragraphs(text);
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].package, control.installables()[0].name());
        assert_eq!(paragraphs[0].field_line("Package"), Some(5));
        assert_eq!(paragraphs[0].field_line("Build-Depends"), Some(7));
        assert!(names_package(
            &build_depends_all_names(control.source()),
            "debhelper"
        ));
    }
}
