use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct EmbeddedPhpLibrary;

declare_rule! {
    /// The package embeds a PHP library that Debian packages.
    ///
    /// A regular file whose path matches lintian's table of well-known PHP
    /// library file names (`/fpdf.php`, `/Snoopy.class.php`, ...) in a package
    /// that is not itself the library. Most patterns ignore case; the TCPDF,
    /// Dompdf, getID3 and gettext ones do not, as in lintian. Extras are
    /// `please use` and the providing package name, verbatim from the table.
    /// Udebs are checked too.
    EmbeddedPhpLibrary,
    code = "LN0652",
    tag = "embedded-php-library",
    default_selected = true,
    experimental = false,
}

/// (providing package regex, path pattern).
const PROVIDERS: &[(&str, &str)] = &[
    ("libphp-adodb", r"(?i)/adodb\.inc\.php$"),
    ("smarty3?", r"(?i)/Smarty(?:_Compiler)?\.class\.php$"),
    (
        "libphp-phpmailer",
        r"(?i)/(phpmailer\.lang-(.*)\.php|PHPMailer\.php)$",
    ),
    (
        "phpsysinfo",
        r"(?i)/phpsysinfo\.dtd|/class\.(?:Linux|(?:Open|Net|Free|)BSD)\.inc\.php$",
    ),
    ("libphp-snoopy", r"(?i)/Snoopy\.class\.(?:php|inc)$"),
    ("php-geshi", r"(?i)/geshi\.php$"),
    (
        "libphp-phplayersmenu",
        r"(?i)/.*layersmenu.*/(lib/)?PHPLIB\.php$",
    ),
    ("libphp-phpsniff", r"(?i)/phpSniff\.(?:class|core)\.php$"),
    ("libphp-jabber", r"(?i)/(?:class\.)?jabber\.php$"),
    (
        "libphp-simplepie",
        r"(?i)/(?:class[\.-])?simplepie(?:\.(?:php|inc))+$",
    ),
    ("libphp-jpgraph", r"(?i)/jpgraph\.php$"),
    ("php-fpdf", r"(?i)/fpdf\.php$"),
    ("php-tcpdf", r"/tcpdf\.php$"),
    ("php-dompdf", r"/Dompdf\.php$"),
    ("php-getid3", r"/(getid3(?:\.lib)?\.php|GetID3\.php)$"),
    ("php-php-gettext", r"/gettext\.inc$"),
    ("php-sparkline", r"(?i)/Sparkline\.php$"),
    (
        "php-econea-nusoap",
        r"(?i)/(?:class\.)?nusoap\.(?:php|inc)$",
    ),
    ("php-htmlpurifier", r"(?i)/HTMLPurifier\.php$"),
];

struct Provider {
    name: &'static str,
    package: Regex,
    file: Regex,
}

static PROVIDER_REGEXES: LazyLock<Vec<Provider>> = LazyLock::new(|| {
    let mut providers: Vec<Provider> = PROVIDERS
        .iter()
        .map(|(name, file)| Provider {
            name,
            package: Regex::new(&format!("^{name}$")).expect("valid regex"),
            file: Regex::new(file).expect("valid regex"),
        })
        .collect();
    providers.sort_by_key(|provider| provider.name);
    providers
});

static INTERESTING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:php|inc|dtd)").expect("valid regex"));

impl BinaryPackageRule for EmbeddedPhpLibrary {
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
            for provider in PROVIDER_REGEXES.iter() {
                if provider.package.is_match(&package) || !provider.file.is_match(name) {
                    continue;
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

    fn tree(files: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|path| (path.to_string(), TestFile::new("<?php")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&["usr/share/example/index.php"]),
            // The library itself.
            tree(&["usr/share/php/fpdf.php"]).package("php-fpdf"),
            tree(&["usr/share/php/Smarty/Smarty.class.php"]).package("smarty3"),
            // Needs a directory component.
            tree(&["fpdf.php"]),
            // These patterns are case-sensitive.
            tree(&["usr/share/php/TCPDF.php", "usr/share/php/DomPDF.php"]),
            tree(&["usr/share/php/Gettext.inc"]),
        ];
        let fail = vec![
            tree(&["usr/share/example/fpdf.php"]),
            tree(&["usr/share/example/FPDF.PHP"]),
            tree(&["usr/share/example/phpmailer.lang-fr.php"]),
            tree(&["usr/share/example/Smarty_Compiler.class.php"]),
            tree(&["usr/share/example/phpsysinfo.dtd"]),
            tree(&[
                "usr/share/example/getid3.lib.php",
                "usr/share/example/GetID3.php",
            ]),
            tree(&["usr/share/example/tcpdf.php"]).udeb(),
            tree(&["usr/share/example/fpdf.php"]).package("php-tcpdf"),
        ];
        BinaryPackageTester::new(EmbeddedPhpLibrary, pass, fail).test_and_snapshot();
    }
}
