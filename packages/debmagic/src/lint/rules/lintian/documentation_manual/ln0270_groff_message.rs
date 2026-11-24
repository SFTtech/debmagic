use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use rayon::prelude::*;

use super::content;

pub struct GroffMessage;

declare_rule! {
    /// `man --warnings` reported a problem in this manual page.
    ///
    /// A regular file or hardlink in `usr/share/man/.../manN/`. Gzip magic
    /// `1f 8b` is decompressed for the line filters; `man` reads the stored
    /// bytes. A page under 256 bytes whose opening line is empty or a `.so`
    /// request is left alone. `man` runs from the directory above `manN`,
    /// with `LC_ALL=C.UTF-8`, an empty `MANROFFSEQ`, and `MANWIDTH=120`, so
    /// a relative `.so` resolves. Other variables are cleared except `PATH`
    /// and `TMPDIR`. Blank stderr, `Reformatting`, a leading `gzip` word,
    /// the old pod2man `Tr`/`IX` warning, and a bracketed can't-break or
    /// cannot-adjust warning on a URL or `.UE` line are dropped. `ja`, `ko`,
    /// and `zh` pages also drop those bracketed wrap warnings. Udebs are
    /// skipped. The extra is the stderr line with one leading `token: ` and
    /// a leading `<standard input>:` removed. The Location line is the
    /// 1-based stderr index, counting lines that were dropped.
    GroffMessage,
    code = "LN0270",
    tag = "groff-message",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GroffMessage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        if ctx.data_files().is_none() {
            return;
        }
        let manual = ctx.shared_once(|ctx| content::ManualPages::from_files(ctx.data_files()));
        let Some(root) = manual.root() else {
            return;
        };
        let pages: Vec<&content::Page> = manual
            .entries
            .iter()
            .map(|(page, _)| page)
            .filter(|page| !page.opening.stops())
            .collect();
        // `collect` keeps input order, so the diagnostics stay in page order.
        let warnings: Vec<Vec<(u32, String)>> = pages
            .par_iter()
            .map(|page| content::man_warnings(root, page))
            .collect();
        for (page, warnings) in pages.iter().zip(warnings) {
            for (line, message) in warnings {
                ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                    path: page.path.clone(),
                    line: Some(line),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const CLEAN: &str = ".TH FOO 1\n.SH NAME\nfoo \\- frobnicate files\n.SH DESCRIPTION\nFoo.\n";
    const DEP: &str = ".TH FOO 1\n.SH NAME\nfoo \\- frobnicate files\n.dep\n";
    const INCLUDE: &str = ".TH INCLUDE 1\n\
        .SH NAME\n\
        include \\- include another page\n\
        .SH DESCRIPTION\n\
        See below.\n\
        .so man1/dep.1\n";

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

    /// The snapshot holds groff 1.25 warnings, which end in the warning
    /// category (`[-w space]`). An older or missing groff prints other text.
    /// Stopgap until debmagic carries a pinned groff of its own.
    fn host_groff_matches_snapshot() -> bool {
        let Ok(output) = std::process::Command::new("groff")
            .arg("--version")
            .output()
        else {
            return false;
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version: Vec<u32> = stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().last())
            .map(|version| {
                version
                    .split('.')
                    .map_while(|part| part.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        version >= vec![1, 25]
    }

    #[test]
    fn test() {
        if !host_groff_matches_snapshot() {
            eprintln!("skipped: the snapshot needs groff 1.25 or newer on the host");
            return;
        }
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/clean.1", CLEAN)]),
            case(vec![gzip("usr/share/man/man1/clean.1.gz", CLEAN)]),
            // A short `.so` returns before `man`, which would otherwise warn.
            case(vec![text(
                "usr/share/man/man1/link.1",
                ".so man1/missing.1\n",
            )]),
            case(vec![text("usr/share/doc/foo.1", DEP)]),
            case(vec![(
                "usr/share/man/man1/dep.1".to_string(),
                TestFile::symlink("other.1"),
            )]),
            case(vec![text("usr/share/man/man1/dep.1", DEP)]).udeb(),
            case(vec![(
                "usr/share/man/man1".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            case(vec![
                text("usr/share/man/man1/dep.1", DEP),
                text("usr/share/man/man1/include.1", INCLUDE),
                text("usr/share/man/de/man1/dep.1", DEP),
                text("usr/share/man/ja/man1/dep.1", DEP),
            ]),
            case(vec![gzip("usr/share/man/man1/dep.1.gz", DEP)]),
            case(vec![(
                "usr/share/man/man1/dep.1".to_string(),
                TestFile::new(DEP).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(GroffMessage, pass, fail).test_and_snapshot();
    }
}
