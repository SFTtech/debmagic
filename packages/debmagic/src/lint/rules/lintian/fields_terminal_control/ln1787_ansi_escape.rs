use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

const ESCAPE: char = '\u{1b}';

pub struct AnsiEscape;

declare_rule! {
    /// A control field value contains an ANSI terminal escape code (an ESC byte).
    ///
    /// Checked on the Subject's control paragraph: the Installation control of a
    /// Binary package (deb, udeb, ddeb); on a Source package the `.dsc`; on a
    /// Source tree the source paragraph of `debian/control` (installable
    /// paragraphs are not checked, as lintian's `processable->fields` is the
    /// source paragraph). Each field whose value holds an ESC byte is reported,
    /// in the order the fields are written, with the field name and its value as
    /// the extra. The Subject is the location, so the Diagnostic has no path or
    /// line. Field names are matched case-insensitively and a value that repeats
    /// a field is checked once per occurrence, as lintian does.
    AnsiEscape,
    code = "LN1787",
    tag = "ansi-escape",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for AnsiEscape {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = escaped_fields(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl SourceRule for AnsiEscape {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => escaped_fields(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| escaped_fields(control.source()))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// `<name> <value>` for each field of `paragraph` whose value contains an ESC.
fn escaped_fields(paragraph: &ControlParagraph) -> Vec<String> {
    paragraph
        .names()
        .filter_map(|name| {
            let value = paragraph.get(name)?;
            value.contains(ESCAPE).then(|| format!("{name} {value}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestCase, TestFile,
    };

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn binary() -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
    }

    fn tree(control: &str) -> TestCase {
        TestCase::source_tree([("debian/control", TestFile::new(control))])
    }

    fn package(dsc_text: &str) -> SourcePackageCase {
        let dsc = Dsc::parse(dsc_text).expect("valid dsc");
        SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc)
    }

    const DSC: &str = "Format: 3.0 (quilt)\nSource: example\nBinary: example\n\
         Architecture: all\nVersion: 1.0-1\nMaintainer: Example <ex@example.com>\n\
         Files:\n d41d8cd98f00b204e9800998ecf8427e 0 example_1.0.orig.tar.gz\n";

    #[test]
    fn binary_fields() {
        let pass = vec![
            binary(),
            binary().field("Description", "plain\n text"),
            binary().field("Section", "[31m is not an escape"),
            binary().udeb().field("Description", "udeb"),
            // A file that holds an ESC is content, not a field.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/README",
                TestFile::new("\u{1b}[31mred\u{1b}[0m"),
            )]),
        ];
        let fail = vec![
            binary().field("Description", "red \u{1b}[31mtext\u{1b}[0m"),
            binary()
                .field("Section", "\u{1b}[1mtool")
                .field("Priority", "optional"),
            binary()
                .field("Description", "\u{1b}[0m")
                .field("Homepage", "https://example.com/\u{1b}"),
        ];
        BinaryPackageTester::new(AnsiEscape, pass, fail).test_and_snapshot_with_suffix("binary");
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            tree(CONTROL),
            // Only the source paragraph is checked here.
            tree(&format!("{CONTROL}Description: \u{1b}[31mx\n")),
            // Invalid UTF-8 `debian/control` has no structured paragraph.
            TestCase::source_tree([(
                "debian/control",
                TestFile::from_bytes(b"Source: \xff\x1b\n".to_vec()),
            )]),
            // ESC in a file other than the control file.
            TestCase::source_tree([
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/README", TestFile::new("\u{1b}[31m")),
            ]),
        ];
        let fail = vec![
            tree(&CONTROL.replace("Maintainer: Example", "Maintainer: \u{1b}[1mExample")),
            tree(&CONTROL.replace("Standards-Version: 4.7.2", "Standards-Version: 4.7.2\u{1b}")),
        ];
        SourceTreeTester::new(AnsiEscape, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package(DSC),
            // The Patched files' control is not the `.dsc` paragraph.
            SourcePackageCase::from(vec![(
                "debian/control",
                TestFile::new(CONTROL.replace("Example", "\u{1b}[1mExample")),
            )])
            .dsc(Dsc::parse(DSC).expect("valid dsc")),
        ];
        let fail = vec![
            package(&DSC.replace("Maintainer: Example", "Maintainer: \u{1b}[1mExample")),
            package(&format!("{DSC}X-Note: \u{1b}[31mred\n")),
        ];
        SourcePackageTester::new(AnsiEscape, pass, fail).test_and_snapshot();
    }
}
