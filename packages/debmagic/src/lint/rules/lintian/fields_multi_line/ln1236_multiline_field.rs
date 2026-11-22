use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct MultilineField;

declare_rule! {
    /// A field that Policy 5.1 and 5.6 limit to one line has a newline in its
    /// value. The Source package's `.dsc` (or the source paragraph of
    /// `debian/control` on a Source tree) is read. Extra is the field name;
    /// Location is empty.
    ///
    /// The banned fields are `Architecture`, `Bugs`, `Changed-By`, `Closes`,
    /// `Date`, `Distribution`, `Dm-Upload-Allowed`, `Essential`, `Format`,
    /// `Homepage`, `Installed-Size`, `Installer-Menu-Item`, `Maintainer`,
    /// `Multi-Arch`, `Origin`, `Package`, `Priority`, `Section`, `Source`,
    /// `Standards-Version`, `Subarchitecture`, `Urgency` and `Version`. Package
    /// relations (`Depends`, ...) may span lines in a Source package, so they
    /// are not checked here. A single trailing newline is not a line break.
    /// Only the Source subject is implemented: lintian's Binary-package side
    /// is not in the recipes this Rule is checked against.
    MultilineField,
    code = "LN1236",
    tag = "multiline-field",
    default_selected = true,
    experimental = false,
}

const ALWAYS_SINGLE: &[&str] = &[
    "Architecture",
    "Bugs",
    "Changed-By",
    "Closes",
    "Date",
    "Distribution",
    "Dm-Upload-Allowed",
    "Essential",
    "Format",
    "Homepage",
    "Installed-Size",
    "Installer-Menu-Item",
    "Maintainer",
    "Multi-Arch",
    "Origin",
    "Package",
    "Priority",
    "Section",
    "Source",
    "Standards-Version",
    "Subarchitecture",
    "Urgency",
    "Version",
];

impl SourceRule for MultilineField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let fields = match ctx.dsc() {
            Some(dsc) => dsc.fields().clone(),
            None => match ctx.debian_control() {
                Some(control) => control.source().clone(),
                None => return,
            },
        };
        for name in fields.names().map(str::to_string).collect::<Vec<_>>() {
            if !ALWAYS_SINGLE
                .iter()
                .any(|banned| banned.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            if is_multiline(&fields, &name) {
                ctx.diagnostic(Diagnostic::error(name));
            }
        }
    }
}

/// Lintian's `untrimmed_value` with one final newline removed, then any
/// remaining newline.
fn is_multiline(fields: &ControlParagraph, name: &str) -> bool {
    let Some(value) = fields.untrimmed_value(name) else {
        return false;
    };
    value.strip_suffix('\n').unwrap_or(value).contains('\n')
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).unwrap()
    }

    const DSC: &str = "Format: 3.0 (native)\n\
         Source: example\n\
         Maintainer: Ex <ex@example.com>\n\
         Binary: example\n\
         Version: 1.0\n";

    #[test]
    fn source_tree() {
        let pass = vec![TestCase::source_tree([(
            "debian/control",
            "Source: example\nMaintainer: Ex <ex@example.com>\nBuild-Depends: a,\n b\n",
        )])];
        let fail = vec![TestCase::source_tree([(
            "debian/control",
            "Source: example\nMaintainer: Ex\n <ex@example.com>\n",
        )])];
        SourceTreeTester::new(MultilineField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", "Source: example\n")]).dsc(dsc(DSC)),
            // Package relations may span lines in a Source package.
            SourcePackageCase::from(vec![("debian/control", "Source: example\n")])
                .dsc(dsc(&format!("{DSC}Build-Depends: a,\n b\n"))),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("debian/control", "Source: example\n")]).dsc(dsc(
                &format!("{DSC}Standards-Version: 4.7.0\n 1\n"),
            )),
            SourcePackageCase::from(vec![("debian/control", "Source: example\n")]).dsc(dsc(
                "Format: 3.0 (native)\nSource: example\nMaintainer: Ex\n <ex@example.com>\nBinary: a\nVersion: 1.0\n",
            )),
        ];
        SourcePackageTester::new(MultilineField, pass, fail).test_and_snapshot();
    }
}
