use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct MaintscriptIncludesMaintScriptParameters;

declare_rule! {
    /// A `debian/*maintscript` line forwards `"$@"` to a maintainer script.
    ///
    /// Direct children of `debian/` named `maintscript` or `*.maintscript`.
    /// A line ending in `-- "$@"` or `-- "${@}"` is reported. Extra is empty.
    /// Location is the file and line. A Source package and a Source tree share
    /// this check.
    MaintscriptIncludesMaintScriptParameters,
    code = "LN0422",
    tag = "maintscript-includes-maint-script-parameters",
    default_selected = true,
    experimental = false,
}

static PARAMETERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"--\s+"\$(?:@|\{@\})"\s*$"#).expect("valid regex"));

impl SourceRule for MaintscriptIncludesMaintScriptParameters {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if !is_maintscript(path) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (index, line) in text.split('\n').enumerate() {
                if PARAMETERS.is_match(line) {
                    hits.push((
                        PathBuf::from(path),
                        u32::try_from(index + 1).unwrap_or(u32::MAX),
                    ));
                }
            }
        }
        hits.sort();
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn is_maintscript(path: &str) -> bool {
    let Some((dir, name)) = path.rsplit_once('/') else {
        return false;
    };
    dir == "debian"
        && (name == "maintscript"
            || name
                .strip_suffix("maintscript")
                .is_some_and(|prefix| prefix.ends_with('.')))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn script(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/maintscript", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/changelog", TestFile::new("no script\n"))],
            script("rm_conffile /etc/correct.conf 0.2~ foo\n"),
            vec![(
                "debian/nested/maintscript",
                TestFile::new("rm_conffile /etc/foo -- \"$@\"\n"),
            )],
        ];
        let fail = vec![
            script(
                "rm_conffile /etc/correct.conf 0.2~ foo\nrm_conffile /etc/incorrect.conf 0.2~ foo -- \"$@\"\nrm_conffile /etc/incorrect.conf 0.2~ foo -- \"${@}\"\nrm_conffile /etc/incorrect.conf 0.2~ foo  --  \"$@\" \n",
            ),
            vec![(
                "debian/example.maintscript",
                TestFile::new("rm_conffile /etc/foo -- \"$@\"\n"),
            )],
        ];
        SourceTreeTester::new(MaintscriptIncludesMaintScriptParameters, pass, fail)
            .test_and_snapshot();
    }
}
