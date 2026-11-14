use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct GirSectionNotLibdevel;

declare_rule! {
    /// A public GIR XML file is not in the `libdevel` section.
    ///
    /// Direct children of `usr/share/gir-1.0` and `usr/lib/<triplet>/gir-1.0`
    /// whose names end in `.gir`. `Section: libdevel` and `oldlibs` are
    /// exempt. A missing section is reported as `NONE`. Extra is the section.
    /// Udebs are checked too.
    GirSectionNotLibdevel,
    code = "LN0435",
    tag = "gir-section-not-libdevel",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GirSectionNotLibdevel {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let section = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
            .unwrap_or_default();
        if matches!(section.as_str(), "libdevel" | "oldlibs") {
            return;
        }
        let extra = if section.is_empty() {
            "NONE".to_string()
        } else {
            section
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                is_gir(path).then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn(extra.clone()).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_gir(path: &str) -> bool {
    let Some(name) = path.strip_prefix("usr/share/gir-1.0/") else {
        return multiarch_gir(path);
    };
    !name.is_empty() && !name.contains('/') && name.ends_with(".gir")
}

fn multiarch_gir(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("usr/lib/") else {
        return false;
    };
    let Some((triplet, name)) = rest.split_once("/gir-1.0/") else {
        return false;
    };
    !triplet.is_empty()
        && !triplet.contains('/')
        && !name.is_empty()
        && !name.contains('/')
        && name.ends_with(".gir")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn packaged(section: Option<&str>, path: &str) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(vec![(path, TestFile::new("<repository/>"))]);
        if let Some(section) = section {
            case = case.field("Section", section);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            packaged(Some("libdevel"), "usr/share/gir-1.0/Foo-1.0.gir"),
            packaged(
                Some("oldlibs"),
                "usr/lib/x86_64-linux-gnu/gir-1.0/Foo-1.gir",
            ),
            packaged(Some("misc"), "usr/share/gir-1.0/nested/Foo.gir"),
            packaged(Some("misc"), "usr/share/doc/Foo.gir"),
        ];
        let fail = vec![
            packaged(Some("misc"), "usr/share/gir-1.0/Bad-23.gir"),
            packaged(Some("libs"), "usr/lib/MULTIARCH/gir-1.0/BadExtras-23.gir"),
            packaged(None, "usr/share/gir-1.0/Foo.gir").udeb(),
        ];
        BinaryPackageTester::new(GirSectionNotLibdevel, pass, fail).test_and_snapshot();
    }
}
