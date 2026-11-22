use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataMalformedModaliasProvide;

declare_rule! {
    /// A `modalias` in AppStream metadata has a lower-case hexadecimal USB ID.
    ///
    /// Checked on each `modalias` of the first `<provides>` of a component in
    /// the metainfo and legacy files. An alias starting `usb:v` with four hex
    /// digits, `p` and four more, and `d`, is reported when those digits are
    /// not all upper case. Extra names the alias. The Location is the file.
    AppstreamMetadataMalformedModaliasProvide,
    code = "LN1465",
    tag = "appstream-metadata-malformed-modalias-provide",
    default_selected = true,
    experimental = false,
}

static USB_ANY_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^usb:v[0-9a-f]{4}p[0-9a-f]{4}d").expect("valid regex"));
static USB_UPPER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usb:v[0-9A-F]{4}p[0-9A-F]{4}d").expect("valid regex"));

impl BinaryPackageRule for AppstreamMetadataMalformedModaliasProvide {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.appstream_files() else {
            return;
        };
        for document in files.all() {
            for alias in document.modaliases() {
                if !USB_ANY_CASE.is_match(alias) || USB_UPPER.is_match(alias) {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::warn(format!(
                        "include non-valid hex digit in USB matching rule '{alias}'"
                    ))
                    .with_location(Location {
                        path: document.path().to_path_buf(),
                        line: None,
                        column: None,
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn metainfo(aliases: &[&str]) -> BinaryPackageCase {
        let nodes: String = aliases
            .iter()
            .map(|alias| format!("<modalias>{alias}</modalias>"))
            .collect();
        BinaryPackageCase::from(vec![(
            "usr/share/metainfo/foo.metainfo.xml".to_string(),
            TestFile::new(format!(
                "<component><provides>{nodes}</provides></component>"
            )),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            metainfo(&["usb:v0000p000Ad*", "usb:v0000p000Ad*x"]),
            metainfo(&["usb:v000Ap0001d*", "pci:v0000p000Ad*"]),
            metainfo(&[]),
        ];
        let fail = vec![
            metainfo(&["usb:v0000p000ad*"]),
            metainfo(&["usb:v0000p000ad*", "usb:v00aap0001dx"]),
        ];
        BinaryPackageTester::new(AppstreamMetadataMalformedModaliasProvide, pass, fail)
            .test_and_snapshot();
    }
}
