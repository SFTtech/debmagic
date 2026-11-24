use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintainerScriptSetsAlternativeImproperly;

declare_rule! {
    /// A maintainer script sets or configures an alternative with
    /// `update-alternatives --set`, `--set-selections`, or `--config`.
    ///
    /// The command must start the logical line; a line that starts with
    /// `echo` and a quoted string is not reported, so a message that names
    /// the command passes. Every maintainer script with an interpreter is
    /// checked, including debhelper automatic sections and here-documents.
    /// Extra is the matched command in double quotes, such as
    /// `"update-alternatives --set"`; the Location is the line where the
    /// logical line ends. The `echo` exception is a negative lookahead in
    /// lintian's pattern and is emulated here, since the `regex` crate has no
    /// lookaround.
    MaintainerScriptSetsAlternativeImproperly,
    code = "LN0987",
    tag = "maintainer-script-sets-alternative-improperly",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\bupdate-alternatives\s+--(?:set|set-selections|config)\b").expect("valid regex")
});

static ECHO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*echo\s+["'][^'"]*"#).expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptSetsAlternativeImproperly {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for line in scan::script_lines(files) {
            if ECHO.is_match(&line.text) {
                continue;
            }
            let Some(found) = COMMAND.find(&line.text) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(format!("\"{}\"", found.as_str().trim())).with_location(
                    Location {
                        path: line.path,
                        line: Some(line.line),
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --display editor\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\nupdate-alternatives --remove editor /usr/bin/vim\n",
            ),
            // A message that names the command is not a call.
            control(
                "postinst",
                "#!/bin/sh\necho \"update-alternatives --set foo\"\n",
            ),
            // A comment is not a command.
            control("postinst", "#!/bin/sh\n# update-alternatives --set foo\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --set editor /usr/bin/vim\n",
            ),
            control("prerm", "#!/bin/sh\nupdate-alternatives --config editor\n"),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --set editor /usr/bin/vim\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptSetsAlternativeImproperly, pass, fail)
            .test_and_snapshot();
    }
}
