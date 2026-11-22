use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct SourcePackageComponentHasLongFileName;

declare_rule! {
    /// A component of the Source package has a file name longer than 80
    /// characters.
    ///
    /// Each name in the `Files` field of the `.dsc` (the third word of every
    /// line) is measured in characters; names longer than 80 are reported
    /// with the name as extra. Reported at the Source package, with no
    /// Location. Only the Source package is checked: a Source tree has no
    /// `.dsc`, so nothing is reported there. Lintian's check of the
    /// `.changes`/binary file name (`package-has-long-file-name`) is a
    /// different Tag and not part of this Rule.
    SourcePackageComponentHasLongFileName,
    code = "LN1288",
    tag = "source-package-component-has-long-file-name",
    default_selected = true,
    experimental = false,
}

const FILENAME_LENGTH_LIMIT: usize = 80;

impl SourceRule for SourcePackageComponentHasLongFileName {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(dsc) = ctx.dsc() else {
            return;
        };
        let Some(files) = dsc.fields().untrimmed_value("Files") else {
            return;
        };
        let mut names = Vec::new();
        for line in files.split('\n') {
            // Lintian trims each line, then splits on single spaces: the
            // name is the third field.
            let line = line.trim();
            let Some(name) = line.split(' ').nth(2) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if name.chars().count() > FILENAME_LENGTH_LIMIT {
                names.push(name.to_string());
            }
        }
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn dsc(files: &str) -> Dsc {
        Dsc::parse(&format!(
            "Format: 3.0 (quilt)\n\
             Source: example\n\
             Version: 1.0-1\n\
             Maintainer: Example <ex@example.com>\n\
             Files:\n{files}"
        ))
        .expect("Dsc fixture")
    }

    fn case(files: &str) -> SourcePackageCase {
        SourcePackageCase::from(vec![("debian/control", CONTROL)]).dsc(dsc(files))
    }

    #[test]
    fn test() {
        // Source tree: no `.dsc`, so nothing to measure.
        let pass = vec![vec![("debian/control", CONTROL)]];
        let fail: Vec<Vec<(&str, &str)>> = vec![];
        SourceTreeTester::new(SourcePackageComponentHasLongFileName, pass, fail).test();
    }

    #[test]
    fn test_source_package() {
        let long_80 = "a".repeat(80);
        let long_81 = "a".repeat(81);
        let pass = vec![
            case(" d41d8cd98f00b204e9800998ecf8427e 0 empty\n"),
            // Exactly at the limit is fine.
            case(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {long_80}\n")),
            // Length counts characters, not bytes.
            case(&format!(
                " d41d8cd98f00b204e9800998ecf8427e 0 {}\n",
                "ä".repeat(80)
            )),
        ];
        let fail = vec![
            case(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {long_81}\n")),
            case(&format!(
                " d41d8cd98f00b204e9800998ecf8427e 0 empty\n d41d8cd98f00b204e9800998ecf8427e 0 {long_81}\n"
            )),
            case(&format!(
                " d41d8cd98f00b204e9800998ecf8427e 0 {}\n",
                "ä".repeat(81)
            )),
        ];
        SourcePackageTester::new(SourcePackageComponentHasLongFileName, pass, fail)
            .test_and_snapshot();
    }
}
