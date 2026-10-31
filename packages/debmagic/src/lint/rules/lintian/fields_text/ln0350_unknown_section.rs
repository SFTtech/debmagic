use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownSection;

declare_rule! {
    /// `Section` is not an archive section, or its area prefix is not one
    /// lintian knows.
    ///
    /// Installation control. The section list is lintian's `fields/archive-sections`.
    /// Area prefixes are `contrib`, `non-free`, and `non-free-firmware`. A
    /// value of exactly `unknown` is the dh_make template Tag, not this one.
    /// A bad area and a bad section each emit the whole field value, so both
    /// can fire. Udebs are not checked. Extra is the field value.
    UnknownSection,
    code = "LN0350",
    tag = "unknown-section",
    default_selected = true,
    experimental = false,
}

/// `data/fields/archive-sections` (lintian 2.139.0).
const SECTIONS: &[&str] = &[
    "admin",
    "cli-mono",
    "comm",
    "database",
    "debian-installer",
    "debug",
    "devel",
    "doc",
    "editors",
    "education",
    "electronics",
    "embedded",
    "fonts",
    "games",
    "gnome",
    "gnu-r",
    "gnustep",
    "golang",
    "graphics",
    "hamradio",
    "haskell",
    "httpd",
    "interpreters",
    "introspection",
    "java",
    "javascript",
    "kde",
    "kernel",
    "libdevel",
    "libs",
    "lisp",
    "localization",
    "mail",
    "math",
    "metapackages",
    "misc",
    "net",
    "news",
    "ocaml",
    "oldlibs",
    "otherosfs",
    "perl",
    "php",
    "python",
    "raku",
    "ruby",
    "rust",
    "science",
    "shells",
    "sound",
    "tasks",
    "tex",
    "text",
    "utils",
    "vcs",
    "video",
    "web",
    "x11",
    "xfce",
    "zope",
];

const ARCHIVE_PARTS: &[&str] = &["non-free", "contrib", "non-free-firmware"];

impl BinaryPackageRule for UnknownSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(section) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
        else {
            return;
        };
        for message in unknown_messages(&section) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn unknown_messages(section: &str) -> Vec<String> {
    let mut parts = section.splitn(2, '/');
    let first = parts.next().unwrap_or("");
    let (division, fraction) = match parts.next() {
        Some(fraction) => (Some(first), fraction),
        None => (None, first),
    };
    let mut messages = Vec::new();
    if division.is_some_and(|area| !ARCHIVE_PARTS.contains(&area)) {
        messages.push(section.to_string());
    }
    let dh_make_template = fraction == "unknown" && division.is_none();
    if !dh_make_template && !SECTIONS.contains(&fraction) {
        messages.push(section.to_string());
    }
    messages
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn section(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Section", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            section("libs"),
            section("devel"),
            section("contrib/libs"),
            section("non-free/misc"),
            section("non-free-firmware/utils"),
            section("unknown"),
            section("debian-installer"),
            section("nonsense").udeb(),
            section("contrib/dveel").udeb(),
        ];
        let fail = vec![
            section("does-not-exist"),
            section("contrib/dveel"),
            section("weird/libs"),
            section("weird/nope"),
            section("Devel"),
        ];
        BinaryPackageTester::new(UnknownSection, pass, fail).test_and_snapshot();
    }
}
