use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataMissingModaliasProvide;

declare_rule! {
    /// A udev rule grants user or group access to a device that AppStream does
    /// not announce.
    ///
    /// The rules are the files in `usr/lib/udev/rules.d/` and
    /// `lib/udev/rules.d/`. A rule line grants access when it names `plugdev`
    /// or `uaccess`, or sets `MODE="0666"`. A USB rule with both an
    /// `idVendor` and an `idProduct` needs a `modalias` in AppStream metadata
    /// starting `usb:v<VENDOR>p<PRODUCT>d` in upper case. Without one, the
    /// Rule reports `match rule usb:v<VENDOR>p<PRODUCT>d*` at the rule's line.
    /// It also reports the file, with no Location, when the rule grants access
    /// and no AppStream file has any `modalias` at all.
    ///
    /// Lines are joined across trailing backslashes and comments are skipped,
    /// as lintian does. The line of a finding is the zero-based index of the
    /// rule's last physical line, the number lintian prints. Non-UTF-8 bytes
    /// are read lossily.
    AppstreamMetadataMissingModaliasProvide,
    code = "LN1466",
    tag = "appstream-metadata-missing-modalias-provide",
    default_selected = true,
    experimental = false,
}

const RULE_DIRS: [&str; 2] = ["usr/lib/udev/rules.d/", "lib/udev/rules.d/"];

static USB_VENDOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"ATTR\{idVendor\}=="([0-9a-fA-F]{4})""#).expect("valid regex"));
static USB_PRODUCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"ATTR\{idProduct\}=="([0-9a-fA-F]{4})""#).expect("valid regex"));

impl BinaryPackageRule for AppstreamMetadataMissingModaliasProvide {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(appstream) = ctx.appstream_files() else {
            return;
        };
        let modaliases: Vec<String> = appstream
            .all()
            .flat_map(|document| document.modaliases().iter().cloned())
            .collect();
        let found_modalias = appstream
            .all()
            .any(|document| document.has_modalias_nodes());

        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut rules: Vec<(PathBuf, String)> = Vec::new();
        for dir in RULE_DIRS {
            for entry in files.entries() {
                if !entry.is_file() || !entry.path().starts_with(dir) {
                    continue;
                }
                let Ok(bytes) = files.read(entry) else {
                    continue;
                };
                rules.push((
                    entry.path().to_path_buf(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
            }
        }

        for (path, text) in rules {
            let mut grants = false;
            let mut found = Vec::new();
            for (position, rule) in rule_lines(&text) {
                let (grant, mismatch) = user_device(rule, &modaliases);
                grants |= grant;
                if let Some(match_rule) = mismatch {
                    found.push((position, match_rule));
                }
            }
            for (position, match_rule) in found {
                ctx.diagnostic(
                    Diagnostic::warn(format!("match rule {match_rule}*")).with_location(Location {
                        path: path.clone(),
                        line: Some(position),
                        column: None,
                    }),
                );
            }
            if grants && !found_modalias {
                ctx.diagnostic(Diagnostic::warn(path.to_string_lossy().into_owned()));
            }
        }
    }
}

/// The logical lines of a udev rule file, each with the zero-based index of
/// its last physical line. Trailing backslashes join lines; comments are
/// dropped.
fn rule_lines(text: &str) -> Vec<(u32, String)> {
    let mut lines = Vec::new();
    let mut continued: Option<String> = None;
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        let mut line = raw.strip_suffix('\n').unwrap_or(raw).to_string();
        if let Some(previous) = continued.take() {
            line = previous + &line;
        }
        if let Some(head) = line.strip_suffix('\\') {
            continued = Some(head.to_string());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        lines.push((index as u32, line));
    }
    lines
}

/// Whether one logical line grants user or group access, and the USB match
/// rule it needs when no `modalias` in `modaliases` covers it.
fn user_device(rule: String, modaliases: &[String]) -> (bool, Option<String>) {
    let grant =
        rule.contains("plugdev") || rule.contains("uaccess") || rule.contains(r#"MODE="0666""#);
    if !rule.contains(r#"SUBSYSTEM=="usb""#) {
        return (grant, None);
    }
    let vendor = USB_VENDOR.captures(&rule).map(|c| c[1].to_uppercase());
    let product = USB_PRODUCT.captures(&rule).map(|c| c[1].to_uppercase());
    let (Some(vendor), Some(product)) = (vendor, product) else {
        return (grant, None);
    };
    let wanted = format!("usb:v{vendor}p{product}d");
    if modaliases.iter().any(|alias| alias.starts_with(&wanted)) {
        (grant, None)
    } else {
        (grant, Some(wanted))
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(rule: &str, metainfo: Option<&str>) -> BinaryPackageCase {
        let mut files = vec![(
            "usr/lib/udev/rules.d/60-foo.rules".to_string(),
            TestFile::new(rule),
        )];
        if let Some(aliases) = metainfo {
            files.push((
                "usr/share/metainfo/foo.metainfo.xml".to_string(),
                TestFile::new(format!(
                    "<component><provides><modalias>{aliases}</modalias></provides></component>"
                )),
            ));
        }
        BinaryPackageCase::from(files)
    }

    #[test]
    fn binary() {
        let usb = r#"SUBSYSTEM=="usb", ATTR{idVendor}=="0000", ATTR{idProduct}=="0001", MODE="0666"
"#;
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            case(r#"KERNEL=="ttyS0", MODE="0660""#, None),
            case("# plugdev in a comment\n", None),
            case(usb, Some("usb:v0000p0001d*")),
            case(
                "SUBSYSTEM==\"usb\", ATTR{idVendor}==\"0000\", \\\n    ATTR{idProduct}==\"0001\", TAG+=\"uaccess\"\n",
                Some("usb:v0000p0001d*"),
            ),
            case(
                r#"SUBSYSTEM=="usb", ATTR{idVendor}=="0000", MODE="0660""#,
                None,
            ),
        ];
        let fail = vec![
            case(usb, None),
            case(usb, Some("usb:v0000p0002d*")),
            case(r#"KERNEL=="foo", GROUP="plugdev""#, None),
            case(
                r#"SUBSYSTEM=="usb", ATTR{idVendor}=="000a", ATTR{idProduct}=="0001", TAG+="uaccess""#,
                Some("usb:v000ap0001d*"),
            ),
        ];
        BinaryPackageTester::new(AppstreamMetadataMissingModaliasProvide, pass, fail)
            .test_and_snapshot();
    }
}
