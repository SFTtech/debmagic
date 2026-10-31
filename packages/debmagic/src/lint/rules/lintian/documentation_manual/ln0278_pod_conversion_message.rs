use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::content;

pub struct PodConversionMessage;

declare_rule! {
    /// The manual page contains a pod2man `POD ERRORS` section.
    ///
    /// A regular file or hardlink in `usr/share/man/.../manN/`. Gzip magic
    /// `1f 8b` is decompressed. A page under 256 bytes whose opening line is
    /// empty or a `.so` request is left alone. A line equal to
    /// `.SH "POD ERRORS"` is flagged; `.\"` comments are skipped and do not
    /// match, but they still count toward the line number. Udebs are
    /// skipped. Extra is empty; the Location is the Data files path and line.
    PodConversionMessage,
    code = "LN0278",
    tag = "pod-conversion-message",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PodConversionMessage {
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
            for line in content::pod_error_lines(&page.lines) {
                hits.push((page.path.clone(), line));
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const POD: &str = ".\\\" comment\n.SH Header \"POD ERRORS\"\n.SH \"POD ERRORS\"\n";

    fn text(path: &str, body: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(body))
    }

    fn gzip(path: &str, body: &str) -> (String, TestFile) {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        (
            path.to_string(),
            TestFile::from_bytes(encoder.finish().expect("gzip")),
        )
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    fn large_so_with_pod() -> String {
        let mut text = ".so man1/foo.1\n".to_string();
        while text.len() < 256 {
            text.push('\n');
        }
        text.push_str(".SH \"POD ERRORS\"\n");
        text
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text(
                "usr/share/man/man1/header.1",
                ".SH Header \"POD ERRORS\"\n",
            )]),
            case(vec![text(
                "usr/share/man/man1/comment.1",
                ".\\\" .SH \"POD ERRORS\"\n",
            )]),
            // A short `.so` returns before the line scan.
            case(vec![text(
                "usr/share/man/man1/link.1",
                ".so man1/foo.1\n.SH \"POD ERRORS\"\n",
            )]),
            case(vec![text("usr/share/doc/foo.1", POD)]),
            case(vec![text("usr/share/man/man1/pod.1", POD)]).udeb(),
        ];
        let fail = vec![
            case(vec![
                text("usr/share/man/man1/pod.1", POD),
                text("usr/share/man/de/man1/pod.1", ".SH \"POD ERRORS\"\n"),
            ]),
            case(vec![gzip("usr/share/man/man1/pod.1.gz", POD)]),
            case(vec![text("usr/share/man/man1/big.1", &large_so_with_pod())]),
        ];
        BinaryPackageTester::new(PodConversionMessage, pass, fail).test_and_snapshot();
    }
}
