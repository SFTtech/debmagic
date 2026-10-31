use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct UdevadmCalledWithoutGuard;

declare_rule! {
    /// A shell maintainer script calls `udevadm` under `set -e` without a guard.
    ///
    /// `udevadm` or `/bin/udevadm` followed by whitespace, after `set -e` or
    /// `set -o errexit` on this line or an earlier one. A line containing
    /// `||` is skipped. `if`, `which`, or `command` before `udevadm` guards
    /// the rest of the file, including that line. `Depends` or `Pre-Depends`
    /// naming `udev` skips the package. `/usr/bin/udevadm` is not this
    /// pattern. Extra is empty. Other interpreters are not scanned.
    UdevadmCalledWithoutGuard,
    code = "LN0303",
    tag = "udevadm-called-without-guard",
    default_selected = true,
    experimental = false,
}

static SET_E: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&scan::leading(
        r"set\s*(?:\s+-(?:-.*|[^e]+))*\s(?:-\w*e|errexit)",
    ))
    .expect("valid regex")
});
static CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&scan::leading(r"(?:/bin/)?udevadm\s")).expect("valid regex"));
static GUARD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:if|which|command)\s+.*udevadm").expect("valid regex"));

impl BinaryPackageRule for UdevadmCalledWithoutGuard {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if strong_mentions_udev(ctx) {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let mut hits = Vec::new();
        let mut current: Option<PathBuf> = None;
        let mut saw_sete = false;
        let mut saw_guard = false;
        for line in scan::script_lines(files)
            .into_iter()
            .filter(|line| line.shell)
        {
            if current.as_ref() != Some(&line.path) {
                current = Some(line.path.clone());
                saw_sete = false;
                saw_guard = false;
            }
            if SET_E.is_match(&line.text) {
                saw_sete = true;
            }
            if GUARD.is_match(&line.text) {
                saw_guard = true;
            }
            if saw_sete && !saw_guard && CALL.is_match(&line.text) && !line.text.contains("||") {
                hits.push((line.path, line.line));
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

/// `Depends` or `Pre-Depends` names `udev`, including as an alternative.
fn strong_mentions_udev(ctx: &BinaryPackageContext<'_>) -> bool {
    let fields = ctx.installation_control().fields();
    ["Pre-Depends", "Depends"].into_iter().any(|field| {
        fields.relation(field).is_some_and(|relation| {
            relation
                .alternatives()
                .any(|alternative| alternative.package() == "udev")
        })
    })
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
            control("postinst", "#!/bin/sh\nudevadm positive\n"),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control("postinst", "#!/bin/sh\nset -e\nudevadm\n"),
            control(
                "postinst",
                "#!/bin/sh\nset -e\nudevadm false-positive || true\nudevadm false-positive || echo warning\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nset -e\nif which udevadm >/dev/null 2>&1; then\n\tudevadm false-positive\nfi\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nset -e\nif [ \"$1\" = configure ] && command -V udevadm >/dev/null 2>&1; then\n\tudevadm false-positive\nfi\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nset -e\nif udevadm false-positive; then\n\tudevadm false-positive\nfi\n",
            ),
            control("postinst", "#!/bin/sh\nset -e\n/usr/bin/udevadm trigger\n"),
            control("postinst", "#!/bin/sh\n# set -e\nudevadm trigger\n"),
            control("postinst", "#!/usr/bin/perl\nset -e\nudevadm trigger\n"),
            control("postinst", "set -e\nudevadm trigger\n"),
            control("triggers", "#!/bin/sh\nset -e\nudevadm trigger\n"),
            control("postinst", "#!/bin/sh\nset -e\nudevadm trigger\n").field("Depends", "udev"),
            control("postinst", "#!/bin/sh\nset -e\nudevadm trigger\n")
                .field("Depends", "${misc:Depends}, udev"),
            control("postinst", "#!/bin/sh\nset -e\nudevadm trigger\n")
                .field("Pre-Depends", "udev (>= 240)"),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nset -e\nudevadm positive\n"),
            control(
                "postinst",
                "#!/bin/sh\nset -e\nudevadm positive\n\nif udevadm false-positive\nthen\n\tudevadm false-positive\nfi\n\nudevadm positive\n",
            ),
            control("prerm", "#!/bin/sh\nset -eu\n/bin/udevadm trigger\n"),
            control("postrm", "#!/bin/sh\nset -o errexit\nudevadm settle\n"),
            control("postinst", "#!/bin/sh\nset -e; udevadm trigger\n"),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nset -e\n# End automatically added section\nudevadm trigger\n",
            ),
            control("postinst", "#!/usr/bin/env bash\nset -e\nudevadm trigger\n")
                .field("Depends", "udev-something"),
            control("postinst", "#!/bin/sh\nset -e\nudevadm one\n")
                .control_file("prerm", TestFile::new("#!/bin/sh\nudevadm two\n")),
        ];
        BinaryPackageTester::new(UdevadmCalledWithoutGuard, pass, fail).test_and_snapshot();
    }
}
