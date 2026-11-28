use std::sync::LazyLock;

use regex::Regex;
use regex::bytes::{Regex as BytesRegex, RegexBuilder as BytesRegexBuilder};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct EmbeddedJavascriptLibrary;

declare_rule! {
    /// The package embeds a JavaScript or CSS library that Debian packages.
    ///
    /// A regular file whose path matches lintian's table of well-known library
    /// file names (`/jquery.min.js`, `/mocha.css`, ...) in a package that is
    /// not itself the library. Embedded Bootstrap is only flagged when the
    /// file contains `var Carousel` or `var Typeahead`. A `jquery.js` next to a
    /// `doxygen.css` is exempt. Extras are `please use` and the providing
    /// package name, verbatim from the table. Udebs are checked too.
    EmbeddedJavascriptLibrary,
    code = "LN0651",
    tag = "embedded-javascript-library",
    default_selected = true,
    experimental = false,
}

/// `[-._]?(compiled|lite|min|...)?.(js|css)(.gz)?` at the end of the path.
const EXT: &str = r"(?:[-._]?(?:compiled|lite|min|pack(?:ed)?|prod|umd|yc)?\.(?:js|css)(?:\.gz)?)$";

/// (providing package regex, path prefix pattern, path ends with `EXT`).
const PROVIDERS: &[(&str, &str, bool)] = &[
    ("ckeditor", "/ckeditor", true),
    ("fckeditor", "/fckeditor", true),
    ("libjs-bootstrap", r"/bootstrap(?:-[\d\.]+)?", true),
    ("libjs-chai", "/chai", true),
    ("libjs-cropper", r"/cropper(?:\.uncompressed)?", true),
    (r"libjs-dojo-\w+", "/(?:dojo|dijit)", true),
    ("libjs-excanvas", "/excanvas(?:-r[0-9]+)?", true),
    ("libjs-jac", "/jsjac", true),
    ("libjs-jquery", r"/jquery(?:-[\d\.]+)?", true),
    ("libjs-jquery-cookie", r"/jquery\.cookie", true),
    ("libjs-jquery-easing", r"/jquery\.easing", true),
    ("libjs-jquery-event-drag", r"/jquery\.event\.drap", true),
    ("libjs-jquery-event-drop", r"/jquery\.event\.drop", true),
    ("libjs-jquery-fancybox", r"/jquery\.fancybox", true),
    ("libjs-jquery-form", r"/jquery\.form", true),
    ("libjs-jquery-galleriffic", r"/jquery\.galleriffic", true),
    ("libjs-jquery-history", r"/jquery\.history", true),
    ("libjs-jquery-jfeed", r"/jquery\.jfeed", true),
    ("libjs-jquery-jush", r"/jquery\.jush", true),
    ("libjs-jquery-livequery", r"/jquery\.livequery", true),
    ("libjs-jquery-meiomask", r"/jquery\.meiomask", true),
    ("libjs-jquery-metadata", r"/jquery\.metadata", true),
    (
        "libjs-jquery-migrate-1",
        r"/jquery-migrate(?:-1[\d\.]*)",
        true,
    ),
    ("libjs-jquery-mousewheel", r"/jquery\.mousewheel", true),
    (
        "libjs-jquery-opacityrollover",
        r"/jquery\.opacityrollover",
        true,
    ),
    ("libjs-jquery-tablesorter", r"/jquery\.tablesorter", true),
    ("libjs-jquery-tipsy", r"/jquery\.tipsy", true),
    ("libjs-jquery-treetable", r"/jquery\.treetable", true),
    ("libjs-jquery-ui", r"/jquery[\.-](?:-[\d\.]+)?ui", true),
    (
        "node-jquery-validation",
        r"/jquery[\.-](?:-[\d\.]+)?validate",
        true,
    ),
    ("libjs-mocha", "/mocha", true),
    ("libjs-mochikit", "/mochikit", true),
    (
        "libjs-mootools",
        r"/mootools(?:(?:\.v|-)[\d\.]+)?(?:-(?:(?:core(?:-server)?)|more)(?:-(?:yc|jm|nc))?)?",
        true,
    ),
    ("libjs-mustache", "/mustache", true),
    ("libjs-normalize", r"/normalize(?:\.min)?\.css", false),
    ("libjs-prototype", r"/prototype(?:-[\d\.]+)?", true),
    ("libjs-raphael", r"/raphael(?:[\.-]min)?", true),
    ("libjs-scriptaculous", "/scriptaculous", true),
    ("libjs-strophe", "/strophe", true),
    ("libjs-underscore", "/underscore", true),
    ("libjs-yui", "/(?:yahoo|yui)-(?:dom-event)?", true),
    ("jsmath", r"/jsMath(?:-fallback-\w+)?", true),
    ("node-html5shiv", "html5shiv(?:-printshiv)?", true),
    (
        "sphinx",
        "/_static/(?:doctools|language_data|searchtools)",
        true,
    ),
    ("tinymce", "/tiny_mce(?:_(?:popup|src))?", true),
    ("libjs-lodash", "lodash", true),
    ("node-async", "/async", true),
    (
        "node-pako",
        r"pako(?:_(:?de|in)flate(?:.es\d+)?)(?:-[\d\.]+)?",
        true,
    ),
    (
        "node-jszip-utils",
        r"jszip-utils(?:-ie)?(?:-[\d\.]+)?",
        true,
    ),
    ("node-jszip", r"jszip(?:-ie)?(?:-[\d\.]+)?", true),
    ("libjs-codemirror", "codemirror", true),
    ("libjs-punycode", r"punycode(?:\.es\d+)?", true),
];

/// Matching one provider pattern against a package or path name.
trait Pattern {
    fn matches(&self, text: &str) -> bool;
}

impl Pattern for Regex {
    fn matches(&self, text: &str) -> bool {
        self.is_match(text)
    }
}

impl Pattern for BytesRegex {
    fn matches(&self, text: &str) -> bool {
        self.is_match(text.as_bytes())
    }
}

struct Provider<R> {
    name: &'static str,
    package: R,
    file: R,
}

/// The table in Unicode mode. Used for a package or path name that is not
/// ASCII, since Unicode case folding and classes can match what ASCII mode
/// does not.
static PROVIDER_REGEXES: LazyLock<Vec<Provider<Regex>>> = LazyLock::new(|| {
    let mut providers: Vec<Provider<Regex>> = PROVIDERS
        .iter()
        .map(|(name, prefix, extension)| Provider {
            name,
            package: Regex::new(&format!("^{name}$")).expect("valid regex"),
            file: Regex::new(&format!(
                "(?i){prefix}{}",
                if *extension { EXT } else { "" }
            ))
            .expect("valid regex"),
        })
        .collect();
    providers.sort_by_key(|provider| provider.name);
    providers
});

/// The table in ASCII mode, for a package and a path that are both ASCII. On
/// ASCII text, every pattern of the table matches exactly what it matches in
/// Unicode mode: `\d`, `\w`, `.`, the classes and `(?i)` agree on ASCII
/// characters, and the only characters that fold to ASCII letters (KELVIN
/// SIGN, LONG S) are not ASCII. ASCII classes are far cheaper to compile. The
/// bytes API is used because `.` in non-Unicode mode may match part of a
/// character, which `regex` rejects for `&str`.
static ASCII_PROVIDER_REGEXES: LazyLock<Vec<Provider<BytesRegex>>> = LazyLock::new(|| {
    let compile = |pattern: &str| {
        BytesRegexBuilder::new(pattern)
            .unicode(false)
            .build()
            .expect("valid regex")
    };
    let mut providers: Vec<Provider<BytesRegex>> = PROVIDERS
        .iter()
        .map(|(name, prefix, extension)| Provider {
            name,
            package: compile(&format!("^{name}$")),
            file: compile(&format!(
                "(?i){prefix}{}",
                if *extension { EXT } else { "" }
            )),
        })
        .collect();
    providers.sort_by_key(|provider| provider.name);
    providers
});

/// The names of the providers whose package and file pattern match, in table
/// order, before the bootstrap magic check.
fn matching_providers<'a, R: Pattern>(
    providers: &'a [Provider<R>],
    package: &'a str,
    name: &'a str,
) -> impl Iterator<Item = &'static str> + 'a {
    providers.iter().filter_map(move |provider| {
        (!provider.package.matches(package) && provider.file.matches(name)).then_some(provider.name)
    })
}

static INTERESTING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:js|css)").expect("valid regex"));
static BOOTSTRAP_MAGIC: LazyLock<BytesRegex> =
    LazyLock::new(|| BytesRegex::new(r"var (?:Carousel|Typeahead)").expect("valid regex"));

impl BinaryPackageRule for EmbeddedJavascriptLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(name) = entry.path().to_str() else {
                continue;
            };
            if !INTERESTING.is_match(name) {
                continue;
            }
            if entry.path().file_name().is_some_and(|n| n == "jquery.js")
                && files
                    .get(entry.path().with_file_name("doxygen.css"))
                    .is_some()
            {
                continue;
            }
            let providers: Vec<&str> = if package.is_ascii() && name.is_ascii() {
                matching_providers(&ASCII_PROVIDER_REGEXES, &package, name).collect()
            } else {
                matching_providers(&PROVIDER_REGEXES, &package, name).collect()
            };
            for provider in providers {
                if provider == "libjs-bootstrap" {
                    let magic = files
                        .read(entry)
                        .is_ok_and(|bytes| BOOTSTRAP_MAGIC.is_match(&bytes));
                    if !magic {
                        continue;
                    }
                }
                found.push((entry.path().to_path_buf(), provider));
            }
        }
        for (path, name) in found {
            ctx.diagnostic(
                Diagnostic::warn(format!("please use {name}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    /// The ASCII table keeps every provider of the Unicode table.
    #[test]
    fn ascii_table_has_every_provider() {
        assert_eq!(
            super::ASCII_PROVIDER_REGEXES.len(),
            super::PROVIDER_REGEXES.len()
        );
    }

    /// A non-ASCII path takes the Unicode table, and is still found.
    #[test]
    fn non_ascii_path_is_found() {
        BinaryPackageTester::new(
            EmbeddedJavascriptLibrary,
            vec![],
            vec![tree(&[("usr/share/exämple/jquery.min.js", "x")])],
        )
        .test();
    }

    use super::*;

    fn tree(files: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("usr/share/example/app.js", "x")]),
            // The library itself.
            tree(&[("usr/share/javascript/jquery/jquery.js", "x")]).package("libjs-jquery"),
            // Needs a directory component before the library name.
            tree(&[("jquery.js", "x")]),
            // Doxygen's bundled jQuery.
            tree(&[
                ("usr/share/doc/example/html/jquery.js", "x"),
                ("usr/share/doc/example/html/doxygen.css", "x"),
            ]),
            // Bootstrap needs the magic string.
            tree(&[("usr/share/example/bootstrap.js", "var x = 1;")]),
            tree(&[("usr/share/example/jquery.min.map", "x")]),
        ];
        let fail = vec![
            tree(&[("usr/share/example/jquery.min.js", "x")]),
            tree(&[("usr/share/example/jquery-1.2.3.js", "x")]),
            tree(&[("usr/share/example/prototype.js.gz", "x")]),
            tree(&[("usr/share/example/Mocha.CSS", "x")]),
            tree(&[("usr/share/example/yahoo-min.js", "x")]),
            tree(&[("usr/share/example/normalize.min.css", "x")]),
            tree(&[("usr/share/example/bootstrap-1.1.js", "var Carousel = 1;")]),
            tree(&[
                ("usr/share/example/jquery.js", "x"),
                ("usr/share/example/tiny_mce_popup.js", "x"),
            ]),
            tree(&[("usr/share/example/jquery.js", "x")]).udeb(),
        ];
        BinaryPackageTester::new(EmbeddedJavascriptLibrary, pass, fail).test_and_snapshot();
    }
}
