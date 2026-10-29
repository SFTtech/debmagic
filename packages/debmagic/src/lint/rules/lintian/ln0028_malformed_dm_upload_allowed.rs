use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln0022_dm_upload_allowed_is_obsolete::dm_upload_allowed_value;

pub struct MalformedDmUploadAllowed;

declare_rule! {
    /// The `DM-Upload-Allowed` field is declared with a value other than `yes`.
    ///
    /// Reads the field as `dm-upload-allowed-is-obsolete` does (the Dsc on a
    /// Source package, the `X[SBC]*-` copy in the source paragraph of
    /// `debian/control` on a Source tree). The unfolded value must be exactly
    /// `yes`, in lower case. Extra is the value. Location is empty.
    MalformedDmUploadAllowed,
    code = "LN0028",
    tag = "malformed-dm-upload-allowed",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MalformedDmUploadAllowed {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if let Some(value) = dm_upload_allowed_value(ctx)
            && value != "yes"
        {
            ctx.diagnostic(Diagnostic::error(value));
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
            control("XS-DM-Upload-Allowed: yes\n"),
            control("XSBC-Dm-Upload-Allowed:  yes \n"),
            // Not copied to the Dsc.
            control("DM-Upload-Allowed: no\n"),
            control("XB-DM-Upload-Allowed: no\n"),
        ];
        let fail = vec![
            control("XS-DM-Upload-Allowed: no\n"),
            control("XS-DM-Upload-Allowed: Yes\n"),
            control("XS-DM-Upload-Allowed: yes please\n"),
            control("XS-DM-Upload-Allowed:\n"),
        ];
        SourceTreeTester::new(MalformedDmUploadAllowed, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |extra: &str| Dsc::parse(&format!("Source: example\n{extra}")).expect("Dsc");
        let pass = vec![
            SourcePackageCase::from(control("")).dsc(dsc("")),
            SourcePackageCase::from(control("")).dsc(dsc("DM-Upload-Allowed: yes\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(control("")).dsc(dsc("DM-Upload-Allowed: no\n")),
            SourcePackageCase::from(control("")).dsc(dsc("DM-Upload-Allowed: Yes\n")),
        ];
        SourcePackageTester::new(MalformedDmUploadAllowed, pass, fail).test_and_snapshot();
    }
}
