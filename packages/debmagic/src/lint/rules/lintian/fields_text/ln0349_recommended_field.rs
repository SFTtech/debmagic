use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct RecommendedField;

declare_rule! {
    /// A recommended field is absent from the source package's primary control.
    ///
    /// The source paragraph of `debian/control` should declare `Section`.
    /// Extra is `(in section for source)` then the field. A Source package's
    /// Dsc should declare `Package-List`; extra is the `.dsc` basename then
    /// the field. Installable paragraphs are not checked (lintian's list for
    /// them is empty). `.changes` `Urgency` is not a Subject here.
    RecommendedField,
    code = "LN0349",
    tag = "recommended-field",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RecommendedField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let mut messages = Vec::new();
        if let Some(dsc) = ctx.dsc()
            && !dsc.fields().declares("Package-List")
        {
            let basename = ctx
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            messages.push(format!("{basename} Package-List"));
        }
        if let Some(control) = ctx.debian_control()
            && !control.source().declares("Section")
        {
            messages.push("(in section for source) Section".to_string());
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(section: bool) -> Vec<(&'static str, TestFile)> {
        let section = if section { "Section: devel\n" } else { "" };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n{section}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

    #[test]
    fn test() {
        let installable_omits_section = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\nSection: devel\n\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
            ),
        )];
        let pass = vec![
            control(true),
            installable_omits_section,
            vec![("debian/control", TestFile::new("not a deb822 file\n: \n"))],
        ];
        // A missing `debian/control` is an empty source paragraph, which does
        // not declare Section.
        let fail = vec![Vec::<(&str, TestFile)>::new(), control(false)];
        SourceTreeTester::new(RecommendedField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let with_section = control(true);
        let pass = vec![
            SourcePackageCase::from(with_section.clone())
                .dsc(dsc("Package-List:\n example deb devel optional arch=all\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(control(false))
                .dsc(dsc("Package-List:\n example deb devel optional arch=all\n")),
            SourcePackageCase::from(with_section).dsc(dsc("Format: 3.0 (native)\n")),
            SourcePackageCase::from(control(false)).dsc(dsc("Format: 3.0 (native)\n")),
        ];
        SourcePackageTester::new(RecommendedField, pass, fail).test_and_snapshot();
    }
}
