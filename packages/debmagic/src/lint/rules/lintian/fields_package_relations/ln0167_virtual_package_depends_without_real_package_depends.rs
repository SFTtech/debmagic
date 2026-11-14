use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::BUILD_DEPENDS_FIELDS;
use super::virtual_packages::is_virtual;

pub struct VirtualPackageDependsWithoutRealPackageDepends;

declare_rule! {
    /// The first alternative of a dependency is a virtual package.
    ///
    /// Binary packages: `Depends` and `Pre-Depends` of the Installation
    /// control. `base-files` may depend on `awk`, and a name starting with
    /// `phpapi-` is left alone. Source: `Build-Depends`, `Build-Depends-Indep`,
    /// and `Build-Depends-Arch` of `debian/control`, for every source package.
    /// Extra is the field and the package name. Applies to debs and udebs.
    VirtualPackageDependsWithoutRealPackageDepends,
    code = "LN0167",
    tag = "virtual-package-depends-without-real-package-depends",
    default_selected = true,
    experimental = false,
}

const BINARY_FIELDS: &[&str] = &["Depends", "Pre-Depends"];

impl SourceRule for VirtualPackageDependsWithoutRealPackageDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            virtual_messages(control.source(), BUILD_DEPENDS_FIELDS, None)
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for VirtualPackageDependsWithoutRealPackageDepends {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            virtual_messages(fields, BINARY_FIELDS, Some(package.as_str()))
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// `binary_package` selects the Installation-control exceptions. `None` is the
/// source check, which has none.
fn virtual_messages(
    paragraph: &ControlParagraph,
    fields: &[&str],
    binary_package: Option<&str>,
) -> Vec<String> {
    fields
        .iter()
        .filter_map(|field| Some((*field, paragraph.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .dependencies()
                .iter()
                .filter_map(|dependency| {
                    let name = dependency.first()?.package();
                    if !is_virtual(name) {
                        return None;
                    }
                    if let Some(package) = binary_package
                        && (name.starts_with("phpapi-")
                            || (package == "base-files" && name == "awk"))
                    {
                        return None;
                    }
                    Some(format!("{field}: {name}"))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn source_named(name: &str, fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: {name}\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![],
            control(""),
            control("Build-Depends: libc6\n"),
            control("Build-Depends: exim4 | mail-transport-agent\n"),
            control("Build-Depends: exim4 (>= 1) | mail-transport-agent\n"),
            control("Build-Conflicts: mail-transport-agent\n"),
            // Versioned data keys are the whole line, so `vim` is not virtual.
            control("Build-Depends: vim\n"),
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nMaintainer: Jane <jane@example.com>\n\
                     Standards-Version: 4.7.2\n\n\
                     Package: example\nArchitecture: all\n\
                     Depends: mail-transport-agent\nDescription: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            control(
                "Build-Depends: mail-transport-agent, mail-transport-agent | exim4, \
                 perl-modules (>= 5), mail-transport-agent:any, phpapi-20210902\n\
                 Build-Depends-Indep: java-compiler\n\
                 Build-Depends-Arch: httpd\n",
            ),
            source_named("base-files", "Build-Depends: awk\n"),
        ];
        SourceTreeTester::new(VirtualPackageDependsWithoutRealPackageDepends, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "libc6")]),
            fields(&[("Depends", "exim4 | mail-transport-agent")]),
            fields(&[("Depends", "exim4 (>= 1) | mail-transport-agent")]),
            fields(&[("Recommends", "mail-transport-agent")]),
            fields(&[("Suggests", "awk")]),
            fields(&[("Conflicts", "mail-transport-agent")]),
            fields(&[("Pre-Depends", "phpapi-20210902")]),
            fields(&[("Depends", "phpapi-20210902")]),
            fields(&[("Pre-Depends", "awk")]).package("base-files"),
            fields(&[("Depends", "awk | gawk")]).package("base-files"),
            fields(&[("Depends", "vim")]),
            fields(&[("Depends", "xorg")]),
            fields(&[("Pre-Depends", "gawk | awk")]),
        ];
        let fail = vec![
            fields(&[
                (
                    "Depends",
                    "mail-transport-agent, httpd | apache2, perl-modules (>= 5), mail-transport-agent:any",
                ),
                ("Pre-Depends", "awk|gawk"),
            ]),
            fields(&[("Depends", "mail-transport-agent (>= 1)")]),
            fields(&[("Depends", "mail-transport-agent")]).package("base-files"),
            fields(&[("Depends", "httpd")]).udeb(),
        ];
        BinaryPackageTester::new(VirtualPackageDependsWithoutRealPackageDepends, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
