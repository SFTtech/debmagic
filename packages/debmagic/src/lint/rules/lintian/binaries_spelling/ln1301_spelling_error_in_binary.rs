use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{binary_exception_words, check_spelling, group_exceptions};

pub struct SpellingErrorInBinary;

declare_rule! {
    /// The `strings` of an ELF Data file contain a misspelling from lintian's
    /// spelling lists.
    ///
    /// Scans the ELF Data files that are regular files or hardlinks, outside
    /// `usr/lib/debug/`, with the printable runs of at least four characters
    /// (`strings --all`). Each misspelling is reported once per lowercase
    /// form, with the file as Location and no line; extra is the word and its
    /// correction. Exceptions are the vendored `binaries/spelling-exceptions`
    /// and the package names, `-` pieces and people names of the Binary
    /// package, as lintian's group exceptions. Repeated words are not checked.
    /// Pedantic and not selected by default, as in lintian. Udebs are checked.
    SpellingErrorInBinary,
    code = "LN1301",
    tag = "spelling-error-in-binary",
    default_selected = false,
    experimental = true,
}

impl BinaryPackageRule for SpellingErrorInBinary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(strings) = ctx.data_strings() else {
            return;
        };
        let mut exceptions = group_exceptions(ctx);
        exceptions.extend(binary_exception_words().map(str::to_string));
        for (path, text) in strings.iter() {
            for (found, correction) in check_spelling(text, &exceptions, false) {
                ctx.diagnostic(
                    Diagnostic::warn(format!("{found} {correction}"))
                        .with_location(Location {
                            path: path.to_path_buf(),
                            line: None,
                            column: None,
                        })
                        .with_severity(Severity::Pedantic),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, bytes: Vec<u8>) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(bytes))])
    }

    fn elf_with(text: &str) -> Vec<u8> {
        TestElf::executable()
            .import("printf", Some("GLIBC_2.2.5"))
            .build()
            .into_iter()
            .chain(text.bytes())
            .collect()
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("usr/bin/a", elf_with("a correct sentence here")),
            // Not an ELF file: no strings are read.
            BinaryPackageCase::from(vec![("usr/bin/a", TestFile::new("teh the"))]),
            // Debug symbols are skipped.
            case("usr/lib/debug/usr/bin/a.debug", elf_with("teh the")),
            // Exceptions of the binary list are not reported.
            case("usr/bin/a", elf_with("teH and tEH")),
        ];
        let fail = vec![
            case("usr/bin/a", elf_with("I iz an exprimental speling error")),
            case("usr/bin/a", elf_with("teh the")).udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInBinary, pass, fail).test_and_snapshot();
    }
}
