use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::gir_paths::{location, triplet_or_placeholder, typelib_files};

pub struct TypelibSectionNotIntrospection;

declare_rule! {
    /// A typelib is in a Binary package whose Section is not `introspection`.
    ///
    /// `Section: introspection` and `oldlibs` are exempt. A missing Section is
    /// reported as `NONE`. Typelibs are the direct children of
    /// `usr/lib/girepository-1.0` and `usr/lib/<triplet>/girepository-1.0`
    /// (the triplet is `${DEB_HOST_MULTIARCH}` when the Architecture has none).
    /// Extra is the Section; the typelib is the Location.
    TypelibSectionNotIntrospection,
    code = "LN0921",
    tag = "typelib-section-not-introspection",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TypelibSectionNotIntrospection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (architecture, section) = {
            let fields = ctx.installation_control().fields();
            (
                fields.unfolded_value("Architecture").unwrap_or_default(),
                fields.unfolded_value("Section").unwrap_or_default(),
            )
        };
        if matches!(section.as_str(), "introspection" | "oldlibs") {
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
        let hits: Vec<_> = typelib_files(files, triplet_or_placeholder(&architecture))
            .into_iter()
            .map(location)
            .collect();
        for hit in hits {
            ctx.diagnostic(Diagnostic::warn(extra.clone()).with_location(hit));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(section: Option<&str>, path: &str) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(vec![(path, TestFile::new("<typelib/>"))])
            .package("libbad")
            .field("Architecture", "amd64");
        if let Some(section) = section {
            case = case.field("Section", section);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            package(
                Some("introspection"),
                "usr/lib/girepository-1.0/Bad-23.typelib",
            ),
            package(
                Some("oldlibs"),
                "usr/lib/x86_64-linux-gnu/girepository-1.0/Bad-23.typelib",
            ),
            package(Some("misc"), "usr/share/gir-1.0/Bad-23.gir"),
        ];
        let fail = vec![
            package(Some("misc"), "usr/lib/girepository-1.0/Bad-23.typelib"),
            package(
                None,
                "usr/lib/x86_64-linux-gnu/girepository-1.0/Bad-23.typelib",
            ),
        ];
        BinaryPackageTester::new(TypelibSectionNotIntrospection, pass, fail).test_and_snapshot();
    }
}
