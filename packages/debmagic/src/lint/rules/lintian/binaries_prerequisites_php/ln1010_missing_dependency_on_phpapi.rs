use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::elf::ElfObjectType;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct MissingDependencyOnPhpapi;

declare_rule! {
    /// A PHP extension does not depend on `phpapi-*`.
    ///
    /// A Binary package that installs a regular file or hardlink whose path
    /// matches `usr/lib/phpN/*.so` (with optional `.N` suffixes, `N` one
    /// digit) and whose File type is an ELF executable or shared object
    /// (`pie executable` included; relocatable objects are not) is a PHP
    /// extension. Its `Pre-Depends` and `Depends` must then contain a
    /// dependency that is exactly `phpapi-` plus digits, word characters or
    /// `+`: a single-alternative clause, without version or architecture
    /// restriction. An `OR` clause does not count. Udebs are skipped. Extra
    /// is empty; there is no Location, as lintian reports on the Subject.
    MissingDependencyOnPhpapi,
    code = "LN1010",
    tag = "missing-dependency-on-phpapi",
    default_selected = true,
    experimental = false,
}

static PHP_EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/php\d/.*\.so(?:\.\d+)*$").expect("regex"));
static PHPAPI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^phpapi-[\d\w+]+$").expect("regex"));

impl BinaryPackageRule for MissingDependencyOnPhpapi {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(elf_files) = ctx.data_elf_files() else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let has_php_extension = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .any(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| PHP_EXTENSION.is_match(path))
                    && elf_files.get(entry.path()).is_some_and(|facts| {
                        matches!(
                            facts.object_type(),
                            ElfObjectType::Executable | ElfObjectType::SharedObject
                        )
                    })
            });
        if !has_php_extension {
            return;
        }
        let depends = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"]);
        let declares_phpapi = depends
            .dependencies()
            .iter()
            .any(|clause| clause.len() == 1 && PHPAPI.is_match(clause[0].text().trim()));
        if !declares_phpapi {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn extension(path: &str, depends: Option<&str>) -> BinaryPackageCase {
        let mut case =
            BinaryPackageCase::from(vec![(path.to_string(), elf(TestElf::shared_object()))]);
        if let Some(depends) = depends {
            case = case.field("Depends", depends);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            extension("usr/lib/php5/foo.so", Some("phpapi-20210902")),
            extension("usr/lib/php8/foo.so.1.2", Some("libfoo, phpapi-20210902")),
            BinaryPackageCase::from(vec![("usr/lib/php5/foo.so", elf(TestElf::shared_object()))])
                .field("Pre-Depends", "phpapi-20210902"),
            // Only `usr/lib/phpN/` directly with a `.so` name counts.
            extension("usr/lib/php/foo.so", None),
            extension("usr/lib/php10/foo.so", None),
            extension("usr/lib/php5/sub/foo.txt", None),
            extension("usr/lib/perl5/foo.so", None),
            // Not an ELF executable or shared object.
            BinaryPackageCase::from(vec![("usr/lib/php5/foo.so", elf(TestElf::relocatable()))]),
            BinaryPackageCase::from(vec![("usr/lib/php5/foo.so", TestFile::new("not elf"))]),
            // Symbolic links are not regular files.
            BinaryPackageCase::from(vec![("usr/lib/php5/foo.so", TestFile::symlink("bar.so"))]),
            // A udeb is skipped.
            extension("usr/lib/php5/foo.so", None).udeb(),
        ];
        let fail = vec![
            extension("usr/lib/php5/foo.so", None),
            extension("usr/lib/php5/foo.so.1.2", Some("libfoo")),
            extension("usr/lib/php5/foo.so", Some("perl")),
            // A version or restriction is not the virtual package.
            extension("usr/lib/php5/foo.so", Some("phpapi-20210902 (>= 1)")),
            extension("usr/lib/php5/foo.so", Some("phpapi-20210902 [amd64]")),
            // An OR clause does not count.
            extension("usr/lib/php5/foo.so", Some("phpapi-20210902 | libfoo")),
            extension("usr/lib/php5/foo.so", Some("${phpapi:Depends}")),
            // Executables count too, and hardlinks.
            BinaryPackageCase::from(vec![(
                "usr/lib/php5/foo.so",
                elf(TestElf::pie_executable()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/php5/bar.so",
                elf(TestElf::shared_object()).hardlink_to("usr/lib/php5/foo.so"),
            )]),
        ];
        BinaryPackageTester::new(MissingDependencyOnPhpapi, pass, fail).test_and_snapshot();
    }
}
