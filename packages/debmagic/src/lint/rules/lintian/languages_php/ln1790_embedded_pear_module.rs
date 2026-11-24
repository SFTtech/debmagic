use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct EmbeddedPearModule;

declare_rule! {
    /// The package embeds a copy of a PEAR module that Debian packages.
    ///
    /// A regular file or hardlink whose path matches one of lintian's PEAR
    /// module paths (`/Auth.php`, `/Log/file.php`, `/DB.php`, ...) and whose
    /// bytes contain `pear/` or `pear.` (case-sensitive, as lintian's
    /// `bytes_match`) is reported once per matching module, with the extra
    /// `please use <package>`. The path is the Location. The providing
    /// package is skipped when it is the package itself. Path patterns are
    /// case-sensitive, except the file extension and the `mime` of
    /// `php-mail-mime`. Some carry lintian's negative lookbehinds: `HTTP` is
    /// not taken after `/Auth`, `DB`, `File`, `IMAP` and `LDAP` not after
    /// `/Container`, and `Socket` not after `/FTP`. A match preceded by that
    /// text is skipped. Udebs are checked too.
    EmbeddedPearModule,
    code = "LN1790",
    tag = "embedded-pear-module",
    default_selected = true,
    experimental = false,
}

/// Lintian's `$PEAR_MAGIC`: the bytes a PEAR module carries.
static PEAR_MAGIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"pear[/.]").expect("valid"));

/// (providing package, path pattern, prefix the match must not follow).
const PROVIDERS: &[(&str, &str, Option<&str>)] = &[
    ("php-auth", r"/Auth(?i:\.php)$", None),
    ("php-auth-http", r"/Auth/HTTP(?i:\.php)$", None),
    (
        "php-benchmark",
        r"/Benchmark/(?:Timer|Profiler|Iterate)(?i:\.php)$",
        None,
    ),
    ("php-http", r"/HTTP(?i:\.php)$", Some("/Auth")),
    ("php-cache", r"/Cache(?i:\.php)$", None),
    ("php-cache-lite", r"/Cache/Lite(?i:\.php)$", None),
    ("php-compat", r"/Compat(?i:\.php)$", None),
    ("php-config", r"/Config(?i:\.php)$", None),
    ("php-crypt-cbc", r"/CBC(?i:\.php)$", None),
    ("php-date", r"/Date(?i:\.php)$", None),
    ("php-db", r"/DB(?i:\.php)$", Some("/Container")),
    ("php-file", r"/File(?i:\.php)$", Some("/Container")),
    (
        "php-log",
        r"(?:/Log/(?:file|error_log|null|syslog|sql\w*)|/Log)(?i:\.php)$",
        None,
    ),
    ("php-mail", r"/Mail(?i:\.php)$", None),
    ("php-mail-mime", r"(?i:/mime(?:Part)?)(?i:\.php)$", None),
    ("php-mail-mimedecode", r"/mimeDecode(?i:\.php)$", None),
    ("php-net-ftp", r"/FTP(?i:\.php)$", None),
    ("php-net-imap", r"/IMAP(?i:\.php)$", Some("/Container")),
    ("php-net-ldap", r"/LDAP(?i:\.php)$", Some("/Container")),
    ("php-net-smtp", r"/SMTP(?i:\.php)$", None),
    ("php-net-socket", r"/Socket(?i:\.php)$", Some("/FTP")),
];

struct Provider {
    package: &'static str,
    pattern: Regex,
    not_after: Option<&'static str>,
}

static COMPILED: LazyLock<Vec<Provider>> = LazyLock::new(|| {
    PROVIDERS
        .iter()
        .map(|&(package, pattern, not_after)| Provider {
            package,
            pattern: Regex::new(pattern).expect("valid PEAR path pattern"),
            not_after,
        })
        .collect()
});

impl Provider {
    /// Whether the stored path `name` is a module this provider ships.
    fn matches(&self, name: &[u8]) -> bool {
        let Some(found) = self.pattern.find(name) else {
            return false;
        };
        match self.not_after {
            Some(prefix) => !name[..found.start()].ends_with(prefix.as_bytes()),
            None => true,
        }
    }
}

/// Whether `name` ends in `.php`, in any ASCII case. Every provider pattern
/// ends in `(?i:\.php)$`, and no non-ASCII character case-folds to `.`, `p` or
/// `h`, so a name without this suffix matches no provider. Checking it first
/// spares the `COMPILED` patterns for the packages without PHP files.
fn ends_with_php(name: &[u8]) -> bool {
    name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(b".php")
}

/// The packages whose modules `name` matches, in `PROVIDERS` order.
fn providers_of(name: &[u8]) -> Vec<&'static str> {
    if !ends_with_php(name) {
        return Vec::new();
    }
    COMPILED
        .iter()
        .filter(|provider| provider.matches(name))
        .map(|provider| provider.package)
        .collect()
}

impl BinaryPackageRule for EmbeddedPearModule {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, &'static str)> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() {
                continue;
            }
            let name = entry.path().as_os_str().as_bytes();
            let providers: Vec<&'static str> = providers_of(name)
                .into_iter()
                .filter(|provider| *provider != package)
                .collect();
            if providers.is_empty() {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if !PEAR_MAGIC.is_match(&bytes) {
                continue;
            }
            for provider in providers {
                hits.push((entry.path().to_path_buf(), provider));
            }
        }
        for (path, provider) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("please use {provider}")).with_location(Location {
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // No PEAR magic in the bytes.
            tree("usr/share/php/Auth.php", "<?php // no magic ?>"),
            // Magic is case-sensitive, and `pearls` is not `pear/` or `pear.`.
            tree("usr/share/php/Auth.php", "require 'PEAR.php';"),
            tree("usr/share/php/Auth.php", "pearls and gems"),
            // A path that matches no module.
            tree("usr/share/php/Foo.php", "require 'pear/Auth.php';"),
            // `/Auth` and `/HTTP` are case-sensitive; the extension is not.
            tree("usr/share/php/auth.php", "pear.php"),
            tree("usr/share/php/AUTH.php", "pear.php"),
            // `/HTTP` after `/Auth`: only php-auth-http, not php-http.
            // `/DB` after `/Container`, `/Socket` after `/FTP`.
            tree("usr/share/php/Container/DB.php", "pear.php"),
            tree("usr/share/php/FTP/Socket.php", "pear.php"),
            // The providing package is the package itself.
            tree("usr/share/php/Auth.php", "pear.php").package("php-auth"),
            // Symlinks and directories are not files.
            BinaryPackageCase::from(vec![(
                "usr/share/php/Auth.php",
                TestFile::symlink("/usr/share/php/Other.php"),
            )]),
            BinaryPackageCase::from(vec![("usr/share/php/Auth.php", TestFile::directory())]),
            // Non-UTF-8 name that matches no module.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/php/Auth\xff.php".to_vec()),
                TestFile::new("pear.php"),
            )]),
        ];
        let fail = vec![
            tree("usr/share/php/Auth.php", "require_once 'pear.php';"),
            tree("usr/share/php/Auth.PHP", "require 'pear/Auth.php';"),
            tree("usr/share/php/Log/file.php", "pear/Log.php"),
            tree("usr/share/php/Mail/mime.php", "pear/mime.php"),
            tree("usr/share/php/Mail/MimePart.php", "pear/mime.php"),
            tree("usr/share/php/HTTP.php", "pear.php"),
            tree("usr/share/php/Auth/HTTP.php", "pear.php"),
            tree("usr/share/php/DB.php", "pear.php"),
            tree("usr/share/php/Socket.php", "pear.php"),
            tree("usr/share/php/Cache/Lite.php", "pear.php"),
            // Binary magic: the bytes need not be UTF-8.
            BinaryPackageCase::from(vec![(
                "usr/share/php/Auth.php",
                TestFile::from_bytes(b"\xff\xfepear/\x00".to_vec()),
            )]),
            // Non-UTF-8 directory name, matching php-cache.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/php/\xff/Cache.php".to_vec()),
                TestFile::new("pear.php"),
            )]),
            // A hardlink is a file to lintian.
            BinaryPackageCase::from(vec![(
                "usr/share/php/Auth.php",
                TestFile::new("pear.php").hardlink(),
            )]),
            // Udebs are checked too.
            tree("usr/share/php/Auth.php", "pear.php").udeb(),
        ];
        BinaryPackageTester::new(EmbeddedPearModule, pass, fail).test_and_snapshot();
    }

    #[test]
    fn provider_paths_match_in_any_ascii_case_of_php() {
        assert_eq!(providers_of(b"usr/share/php/Auth.php"), ["php-auth"]);
        assert_eq!(providers_of(b"usr/share/php/Auth.PhP"), ["php-auth"]);
        assert_eq!(
            providers_of(b"usr/share/php/Auth.php.bak"),
            Vec::<&str>::new()
        );
        assert_eq!(providers_of(b"usr/share/php/Auth.phpx"), Vec::<&str>::new());
        assert_eq!(providers_of(b"usr/share/php/Auth"), Vec::<&str>::new());
        assert_eq!(providers_of(b"php"), Vec::<&str>::new());
        assert!(ends_with_php(b".php"));
        assert!(!ends_with_php(b"php"));
    }

    #[test]
    fn provider_paths_are_not_fooled_by_non_ascii_names() {
        // `/Auth` needs a `/` right before it; an accented letter is not one.
        assert!(providers_of("usr/share/php/\u{e9}Auth.php".as_bytes()).is_empty());
        assert!(providers_of("usr/share/php/Auth.php\u{e9}".as_bytes()).is_empty());
        // Lookalike Unicode letters do not change `Socket`, which is case-sensitive.
        assert!(providers_of("usr/share/php/FTP/SOCKET.php".as_bytes()).is_empty());
        assert_eq!(
            providers_of("usr/share/php/FTP/Socket.php".as_bytes()),
            Vec::<&str>::new()
        );
        assert_eq!(
            providers_of(b"usr/share/php/Auth/HTTP.php"),
            ["php-auth-http"]
        );
    }
}
