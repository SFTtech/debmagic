//! OCaml development files (`*.cmi`, `*.cmx`, `*.cmxa`) and lintian's summary
//! of them, shared by the `languages/ocaml/byte-code/misplaced` Rules.

use std::sync::LazyLock;

use regex::Regex;

static DEV_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.cm(?:i|xa?)$").expect("valid regex"));

/// Whether `name` is a `*.cmi`, `*.cmx` or `*.cmxa` file. `*.cma`, `*.cmo` and
/// `*.cmxs` are left out: they can be plugins.
pub(super) fn is_dev_file(name: &str) -> bool {
    DEV_FILE.is_match(name)
}

/// Whether a binary package with this name is a development package, as the
/// misplaced-package Check decides it.
pub(super) fn is_dev_package(name: &str) -> bool {
    name.ends_with("-dev")
        || matches!(
            name,
            "camlp4"
                | "camlp4-extra"
                | "camlp5"
                | "camlp5-extra"
                | "ocaml"
                | "ocaml-nox"
                | "ocaml-interp"
                | "ocaml-compiler-libs"
        )
}

/// lintian's `"<count> file(s) in <prefix>"`, where the prefix is the longest
/// directory that every path is under, or `/` when there is none.
pub(super) fn summary<'a>(paths: impl IntoIterator<Item = &'a str>) -> String {
    let directories: Vec<Vec<&str>> = paths
        .into_iter()
        .map(|path| {
            let mut parts: Vec<&str> = path.split('/').collect();
            parts.pop();
            parts
        })
        .collect();
    let count = directories.len();
    let plural = if count == 1 { "" } else { "s" };
    let mut common = directories.first().cloned().unwrap_or_default();
    for directory in directories.iter().skip(1) {
        let shared = common
            .iter()
            .zip(directory)
            .take_while(|(a, b)| a == b)
            .count();
        common.truncate(shared);
    }
    let prefix = if common.is_empty() {
        "/".to_string()
    } else {
        common.join("/")
    };
    format!("{count} file{plural} in {prefix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_names_the_longest_common_directory() {
        assert_eq!(
            summary(["usr/lib/ocaml/a/x.cmi"]),
            "1 file in usr/lib/ocaml/a"
        );
        assert_eq!(
            summary(["usr/lib/ocaml/a/x.cmi", "usr/lib/ocaml/b/y.cmx"]),
            "2 files in usr/lib/ocaml"
        );
        assert_eq!(
            summary(["usr/lib/x.cmi", "usr/bin/y.cmi"]),
            "2 files in usr"
        );
        assert_eq!(summary(["x.cmi", "usr/y.cmi"]), "2 files in /");
    }

    #[test]
    fn dev_files_and_packages() {
        assert!(is_dev_file("usr/lib/ocaml/a.cmi"));
        assert!(is_dev_file("usr/lib/ocaml/a.cmxa"));
        assert!(!is_dev_file("usr/lib/ocaml/a.cma"));
        assert!(!is_dev_file("usr/lib/ocaml/a.cmo"));
        assert!(is_dev_package("libfoo-dev"));
        assert!(is_dev_package("ocaml-nox"));
        assert!(!is_dev_package("libfoo"));
        assert!(!is_dev_package("ocaml-dev-tools"));
    }
}
