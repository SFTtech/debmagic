use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::content;

pub struct FSSTNDDirInManualPage;

declare_rule! {
    /// A manual page names a directory from the FSSTND layout.
    ///
    /// Pages are read as in `documentation/manual`: gzip is decompressed, and a
    /// page under 256 bytes whose opening line is empty or a `.so` request is
    /// left alone. Lines starting with `.\"` are comments: they are skipped
    /// but still count toward the line number. The first match on a line is
    /// reported, `/usr/(dict|doc|etc|info|man|adm|preserve)/` before
    /// `/var/(adm|catman|named|nis|preserve)/`, as lintian does. Extra is the
    /// matched directory; the line is the Location. Udebs are skipped.
    /// Reported at info severity.
    FSSTNDDirInManualPage,
    code = "LN1354",
    tag = "FSSTND-dir-in-manual-page",
    default_selected = false,
    experimental = false,
}

static USR_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/usr/(?:dict|doc|etc|info|man|adm|preserve)/").expect("valid regex")
});
static VAR_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/var/(?:adm|catman|named|nis|preserve)/").expect("valid regex"));

impl BinaryPackageRule for FSSTNDDirInManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for page in content::pages(files) {
            if page.opening.stops() {
                continue;
            }
            for (index, line) in page.lines.iter().enumerate() {
                if line.starts_with(r#".\""#) {
                    continue;
                }
                let found = USR_DIR.find(line).or_else(|| VAR_DIR.find(line));
                if let Some(found) = found {
                    hits.push((
                        page.path.clone(),
                        u32::try_from(index + 1).unwrap_or(u32::MAX),
                        found.as_str().to_string(),
                    ));
                }
            }
        }
        for (path, line, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn page(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/share/man/man1/example.1", TestFile::new(text))])
            .field("Architecture", "any")
    }

    #[test]
    fn test() {
        let pass = vec![
            page(".TH EXAMPLE 1\n.SH NAME\nsee /usr/share/doc/example\n"),
            page(".TH EXAMPLE 1\n.\\\" see /usr/doc/example\n"),
        ];
        let fail = vec![
            page(".TH EXAMPLE 1\n.SH FILES\n/usr/doc/example/README\n"),
            page(".TH EXAMPLE 1\n.SH FILES\n/var/adm/example\n"),
            // Both patterns on one line: the /usr/ pattern is preferred.
            page(
                ".TH EXAMPLE 1
.SH FILES
/var/adm/example and /usr/doc/x
",
            ),
        ];
        BinaryPackageTester::new(FSSTNDDirInManualPage, pass, fail).test_and_snapshot();
    }
}
