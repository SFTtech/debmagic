use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct MaintainerAlsoInUploaders;

declare_rule! {
    /// The Maintainer value appears within the Uploaders value.
    ///
    /// A substring test, not a parse of the list: the Maintainer's text
    /// anywhere in Uploaders counts. Empty runs of commas (`,\s*,`) in
    /// Uploaders are collapsed to one comma first. Extra is empty. Applies to
    /// the Source tree and Source package.
    MaintainerAlsoInUploaders,
    code = "LN0776",
    tag = "maintainer-also-in-uploaders",
    default_selected = true,
    experimental = false,
}

static DOUBLE_COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*,").expect("valid regex"));

impl SourceRule for MaintainerAlsoInUploaders {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let fields = match ctx.dsc() {
            Some(dsc) => dsc.fields(),
            None => match ctx.debian_control() {
                Some(control) => control.source(),
                None => return,
            },
        };
        let (Some(maintainer), Some(uploaders)) = (
            fields.unfolded_value("Maintainer"),
            fields.unfolded_value("Uploaders"),
        ) else {
            return;
        };
        let uploaders = DOUBLE_COMMA.replace_all(&uploaders, ",");
        if uploaders.contains(maintainer.as_str()) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(maintainer: &str, uploaders: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let uploaders = uploaders
            .map(|value| format!("Uploaders: {value}\n"))
            .unwrap_or_default();
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: {maintainer}\n{uploaders}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("A <a@example.com>", None),
            control("A <a@example.com>", Some("B <b@example.com>")),
            // Not the same text.
            control("A <a@example.com>", Some("A <a@example.org>")),
            control(
                "A <a@example.com>",
                Some("Ann <a@example.com.au>, B <b@example.com>"),
            ),
        ];
        let fail = vec![
            control("A <a@example.com>", Some("A <a@example.com>")),
            control(
                "A <a@example.com>",
                Some("B <b@example.com>, A <a@example.com>"),
            ),
            // Substring match: the Maintainer text inside a longer value.
            control("A <a@example.com>", Some("Mr A <a@example.com>")),
            // A double comma is collapsed before the test.
            control(
                "A <a@example.com>",
                Some("B <b@example.com>,, A <a@example.com>"),
            ),
        ];
        SourceTreeTester::new(MaintainerAlsoInUploaders, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |maintainer: &str, uploaders: &str| {
            Dsc::parse(&format!(
                "Source: example\nMaintainer: {maintainer}\nUploaders: {uploaders}\n"
            ))
            .expect("Dsc")
        };
        let pass = vec![
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new())
                .dsc(dsc("A <a@example.com>", "B <b@example.com>")),
            // The Dsc decides: debian/control's Uploaders is not read.
            SourcePackageCase::from(control("A <a@example.com>", Some("A <a@example.com>")))
                .dsc(dsc("A <a@example.com>", "B <b@example.com>")),
        ];
        let fail = vec![
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new())
                .dsc(dsc("A <a@example.com>", "A <a@example.com>")),
        ];
        SourcePackageTester::new(MaintainerAlsoInUploaders, pass, fail).test_and_snapshot();
    }
}
