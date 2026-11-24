use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{Severity, SourceRule};

pub struct DebianWatchContainsDhMakeTemplate;

declare_rule! {
    /// `debian/watch` still holds the template that `dh_make` installs.
    ///
    /// Fires when the file has a line `# Example watch control file for uscan`
    /// (a comment, case-insensitive) or, failing that, contains `<project>`.
    /// Extra is the matched text: `Example watch control file for uscan`, or
    /// `<project>`. The file is the Location, with no line. Info, not selected
    /// by default. The file is read when it is a regular file or hardlink.
    DebianWatchContainsDhMakeTemplate,
    code = "LN1536",
    tag = "debian-watch-contains-dh_make-template",
    default_selected = false,
    experimental = false,
}

static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?mi)^\s*#\s*(Example watch control file for uscan)").expect("regex")
});
static PROJECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(<project>)").expect("regex"));

impl SourceRule for DebianWatchContainsDhMakeTemplate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((path, text)) = ctx.files().and_then(read_watch) else {
            return;
        };
        let Some(template) = template_text(&text) else {
            return;
        };
        ctx.diagnostic(
            Diagnostic::warn(template)
                .with_severity(Severity::Info)
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
        );
    }
}

/// The `debian/watch` file and its text, when it is a regular file or hardlink.
fn read_watch(files: &FileTree) -> Option<(PathBuf, String)> {
    let bytes = files.read_file("debian/watch")?;
    Some((
        PathBuf::from("debian/watch"),
        String::from_utf8_lossy(&bytes).into_owned(),
    ))
}

/// The first `dh_make` template text, the uscan example header before
/// `<project>`.
fn template_text(text: &str) -> Option<String> {
    HEADER
        .captures(text)
        .or_else(|| PROJECT.captures(text))
        .map(|captures| captures[1].to_string())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn watch(contents: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/watch", TestFile::new(contents))]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            watch("version=4\nhttps://example.org/ .*/v?(\\d\\S+)\\.tar\\.gz\n"),
            // The header needs the whole phrase and must be a comment.
            watch("version=4\n# Example watch control file\n"),
            watch("Example watch control file for uscan\n"),
        ];
        let fail = vec![
            watch("# Example watch control file for uscan\nversion=4\n"),
            // Case does not matter for the header.
            watch("  # EXAMPLE WATCH CONTROL FILE FOR USCAN\n"),
            watch("version=4\nhttps://github.com/<project>/tags .*\n"),
            // The header wins over `<project>` even when it comes later.
            watch("https://github.com/<project>/tags .*\n# Example watch control file for uscan\n"),
        ];
        SourceTreeTester::new(DebianWatchContainsDhMakeTemplate, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(watch("version=4\n"))];
        let fail = vec![SourcePackageCase::from(watch(
            "# Example watch control file for uscan\n",
        ))];
        SourcePackageTester::new(DebianWatchContainsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
