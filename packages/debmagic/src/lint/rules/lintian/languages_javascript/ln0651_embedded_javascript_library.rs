use std::sync::LazyLock;

use regex::{Regex, bytes::Regex as BytesRegex};

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

struct Provider {
    name: &'static str,
    package: Regex,
    file: Regex,
}

static PROVIDER_REGEXES: LazyLock<Vec<Provider>> = LazyLock::new(|| {
    let mut providers: Vec<Provider> = PROVIDERS
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
            for provider in PROVIDER_REGEXES.iter() {
                if provider.package.is_match(&package) || !provider.file.is_match(name) {
                    continue;
                }
                if provider.name == "libjs-bootstrap" {
                    let magic = files
                        .read(entry)
                        .is_ok_and(|bytes| BOOTSTRAP_MAGIC.is_match(&bytes));
                    if !magic {
                        continue;
                    }
                }
                found.push((entry.path().to_path_buf(), provider.name));
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
