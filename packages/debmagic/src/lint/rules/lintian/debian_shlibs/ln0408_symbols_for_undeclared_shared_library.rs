use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::lines::{is_blank_or_comment, perl_lines, read_text};

pub struct SymbolsForUndeclaredSharedLibrary;

declare_rule! {
    /// A symbols file names a versioned library the shlibs file does not.
    ///
    /// Sonames in `symbols` are turned into `name version` the way
    /// `dpkg-shlibdeps` splits them. Shlibs lines use the same `name version`
    /// key, with a `udeb: ` prefix kept on udeb lines. The Tag fires only when
    /// the shlibs file has at least one such key, and only for symbols sonames
    /// that contain a space after the split. An ELF SONAME is not read. Extra
    /// is `name version`. There is no Location.
    SymbolsForUndeclaredSharedLibrary,
    code = "LN0408",
    tag = "symbols-for-undeclared-shared-library",
    default_selected = true,
    experimental = false,
}

static SONAME_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("regex")
});
static SO_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)\.so\.(.*)$").expect("regex"));
static DASH_SO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)-(\d.*)\.so$").expect("regex"));

impl BinaryPackageRule for SymbolsForUndeclaredSharedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(shlibs) = read_text(ctx, "shlibs") else {
            return;
        };
        let declared = shlibs_sonames(&shlibs);
        if declared.is_empty() {
            return;
        }
        let Some(symbols) = read_text(ctx, "symbols") else {
            return;
        };
        let mut missing: Vec<String> = symbols_sonames(&symbols)
            .into_iter()
            .filter(|soname| soname.contains(' ') && !declared.contains(soname))
            .collect();
        missing.sort();
        missing.dedup();
        for soname in missing {
            ctx.diagnostic(Diagnostic::error(soname));
        }
    }
}

fn shlibs_sonames(text: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in perl_lines(text) {
        if is_blank_or_comment(line) {
            continue;
        }
        let (prefix, body) = strip_udeb(line);
        let mut fields = body.split_whitespace();
        let Some(name) = fields.next() else {
            continue;
        };
        let version = fields.next().unwrap_or("");
        names.insert(format!("{prefix}{name} {version}"));
    }
    names
}

fn strip_udeb(line: &str) -> (&str, &str) {
    let Some(rest) = line.strip_prefix("udeb:") else {
        return ("", line);
    };
    if rest.starts_with(char::is_whitespace) {
        ("udeb: ", rest.trim_start())
    } else {
        ("", line)
    }
}

fn symbols_sonames(text: &str) -> Vec<String> {
    perl_lines(text)
        .into_iter()
        .filter(|line| !is_blank_or_comment(line))
        .filter_map(|line| SONAME_LINE.captures(line))
        .map(|captures| human_soname(&captures[1]))
        .collect()
}

fn human_soname(string: &str) -> String {
    if let Some(captures) = SO_VERSION.captures(string) {
        return format!("{} {}", &captures[1], &captures[2]);
    }
    if let Some(captures) = DASH_SO.captures(string) {
        return format!("{} {}", &captures[1], &captures[2]);
    }
    string.to_string()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(shlibs: &str, symbols: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("shlibs", TestFile::new(shlibs))
            .control_file("symbols", TestFile::new(symbols))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control(
                "libbaz 2 libbaz1\n",
                "libbaz.so.2 libbaz1 #MINVER#\n foo 1.0\n",
            ),
            control("", "libbaz.so.2 libbaz1 #MINVER#\n foo 1.0\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("symbols", TestFile::new("libbaz.so.2 libbaz1\n")),
        ];
        let fail = vec![
            control(
                "libdoesntexist2 1.0 libbaz1\nlibbaz2 1.1 libbaz\n",
                "libbaz.so.2 libbaz1 #MINVER#\n foo 1.0\n",
            ),
            control(
                "libhello 1 libhello0\n",
                "libhello.so.2 libhello0 #MINVER#\n bar 1\nlibhello.so.1 libhello0\n baz 1\n",
            ),
        ];
        BinaryPackageTester::new(SymbolsForUndeclaredSharedLibrary, pass, fail).test_and_snapshot();
    }
}
