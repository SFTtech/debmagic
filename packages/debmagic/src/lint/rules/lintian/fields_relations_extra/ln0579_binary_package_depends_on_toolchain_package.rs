use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::super::fields_relations_more::{BINARY_DEP_FIELDS, is_transitional_or_meta};
use super::alternatives_of;

pub struct BinaryPackageDependsOnToolchainPackage;

declare_rule! {
    /// A binary package depends on a toolchain package.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests` against
    /// lintian's `fields/toolchain` list (`debhelper`). Skipped for packages
    /// named `dh-*`, `*-source`, `*-src`, or one of the packages installing a
    /// debhelper add-on, and for transitional or meta packages (Description,
    /// Section, or a `task-` name). Extra is the field and the alternative
    /// as written. Applies to debs and udebs.
    BinaryPackageDependsOnToolchainPackage,
    code = "LN0579",
    tag = "binary-package-depends-on-toolchain-package",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `data/fields/toolchain`.
const TOOLCHAIN: &[&str] = &["debhelper"];

/// Packages that install a debhelper add-on (`installed_by` of
/// `data/debhelper/add_ons.json`, pinned to lintian 2.139.0).
const ADDON_PACKAGES: &[&str] = &[
    "apache2-dev",
    "asterisk-dev",
    "autotools-dev",
    "bash-completion",
    "cli-common-dev",
    "click-dev",
    "cme",
    "config-package-dev",
    "debhelper",
    "debhelper-compat",
    "dh-acc",
    "dh-ada-library",
    "dh-autoreconf",
    "dh-builtusing",
    "dh-cargo",
    "dh-cmake",
    "dh-coq",
    "dh-cruft",
    "dh-debputy",
    "dh-di",
    "dh-dist-zilla",
    "dh-dkms",
    "dh-dlopenlibdeps",
    "dh-elpa",
    "dh-fortran",
    "dh-go",
    "dh-golang",
    "dh-haskell",
    "dh-linktree",
    "dh-lisp",
    "dh-lua",
    "dh-nodejs",
    "dh-nss",
    "dh-ocaml",
    "dh-octave",
    "dh-php",
    "dh-puppet",
    "dh-puredata",
    "dh-python",
    "dh-r",
    "dh-raku",
    "dh-rebar",
    "dh-runit",
    "dh-rust",
    "dh-sequence-phpcomposer",
    "dh-sequence-phppear",
    "dh-sequence-single-binary",
    "dh-setup-copyright",
    "dh-shell-completions",
    "dh-sysuser",
    "dh-user-session-migration",
    "dh-vim-addon",
    "dh-virtualenv",
    "dictionaries-common-dev",
    "doxygen",
    "gem2deb",
    "germinate",
    "gnome-pkg-tools",
    "gobject-introspection-bin",
    "javahelper",
    "kodi-addons-dev-common",
    "libdbi-perl",
    "libdebhelper-perl",
    "libgrantlee5-dev",
    "libimager-perl",
    "libva-dev",
    "libvarnishapi-dev",
    "libxine2-dev",
    "maven-repo-helper",
    "mkdocs",
    "nginx-dev",
    "pdl",
    "perl-openssl-defaults",
    "pkg-kde-tools",
    "pkg-php-tools",
    "postgresql-common-dev",
    "ppp-dev",
    "pyppd",
    "python3-numpy-dev",
    "quilt",
    "scour",
    "sphinx-common",
    "tex-common",
    "uwsgi-dev",
    "vdr-dev",
    "xml-core",
    "xserver-xorg-dev",
    "zathura-dev",
];

impl BinaryPackageRule for BinaryPackageDependsOnToolchainPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            if package.starts_with("dh-")
                || package.ends_with("-source")
                || package.ends_with("-src")
                || ADDON_PACKAGES.contains(&package.as_str())
                || is_transitional_or_meta(&package, fields)
            {
                Vec::new()
            } else {
                alternatives_of(fields, BINARY_DEP_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| TOOLCHAIN.contains(&alternative.package()))
                    .map(|(field, alternative)| format!("{field}: {}", alternative.text()))
                    .collect()
            }
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::testing::binary;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "debhelper-compat, dh-python")]),
            binary(&[("Conflicts", "debhelper")]),
            binary(&[("Depends", "debhelper")]).package("dh-example"),
            binary(&[("Depends", "debhelper")]).package("example-source"),
            binary(&[("Depends", "debhelper")]).package("example-src"),
            binary(&[("Depends", "debhelper")]).package("quilt"),
            binary(&[("Depends", "debhelper")]).field("Description", "A transitional package"),
            binary(&[("Depends", "debhelper")]).field("Section", "metapackages"),
        ];
        let fail = vec![
            binary(&[("Depends", "debhelper (>= 9)")]),
            binary(&[
                ("Pre-Depends", "foo | debhelper"),
                ("Suggests", "debhelper:any"),
            ]),
            binary(&[("Recommends", "debhelper")]).udeb(),
        ];
        BinaryPackageTester::new(BinaryPackageDependsOnToolchainPackage, pass, fail)
            .test_and_snapshot();
    }
}
