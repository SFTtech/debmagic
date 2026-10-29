use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct DmUploadAllowedIsObsolete;

declare_rule! {
    /// The source declares the obsolete `DM-Upload-Allowed` field.
    ///
    /// On a Source package the field is read from the Dsc. A Source tree has
    /// no Dsc, so the source paragraph of `debian/control` stands in for it:
    /// `dpkg-source` copies `X[SBC]*-` fields that carry `S` into the Dsc with
    /// the prefix removed, so `XS-DM-Upload-Allowed` and its permutations
    /// count. Field names are case-insensitive. Extra and Location are empty.
    DmUploadAllowedIsObsolete,
    code = "LN0022",
    tag = "dm-upload-allowed-is-obsolete",
    default_selected = true,
    experimental = false,
}

/// `X`, then some of `S`, `B`, `C` including `S`, then `-`.
const SOURCE_PREFIXES: &[&str] = &[
    "XS-", "XSB-", "XBS-", "XSC-", "XCS-", "XSBC-", "XSCB-", "XBSC-", "XBCS-", "XCSB-", "XCBS-",
];

fn control_copies_to_dsc(source: &ControlParagraph, field: &str) -> bool {
    SOURCE_PREFIXES
        .iter()
        .any(|prefix| source.declares(&format!("{prefix}{field}")))
}

impl SourceRule for DmUploadAllowedIsObsolete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let declared = match ctx.dsc() {
            Some(dsc) => dsc.fields().declares("DM-Upload-Allowed"),
            None => ctx.debian_control().is_some_and(|control| {
                control_copies_to_dsc(control.source(), "DM-Upload-Allowed")
            }),
        };
        if declared {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n{fields}Standards-Version: 4.7.2\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control(""),
            // Not copied to the Dsc.
            control("DM-Upload-Allowed: yes\n"),
            control("XB-DM-Upload-Allowed: yes\n"),
            control("XC-DM-Upload-Allowed: yes\n"),
        ];
        let fail = vec![
            control("XS-DM-Upload-Allowed: yes\n"),
            control("XS-Dm-Upload-Allowed: no\n"),
            control("XSBC-DM-Upload-Allowed: yes\n"),
            control("XCS-DM-Upload-Allowed:\n"),
        ];
        SourceTreeTester::new(DmUploadAllowedIsObsolete, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |extra: &str| Dsc::parse(&format!("Source: example\n{extra}")).expect("Dsc");
        let pass = vec![SourcePackageCase::from(control("")).dsc(dsc(""))];
        let fail = vec![
            SourcePackageCase::from(control("")).dsc(dsc("DM-Upload-Allowed: yes\n")),
            SourcePackageCase::from(control("")).dsc(dsc("Dm-Upload-Allowed: no\n")),
        ];
        SourcePackageTester::new(DmUploadAllowedIsObsolete, pass, fail).test_and_snapshot();
    }
}
