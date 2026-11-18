use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

pub struct MissingDependencyOnPerlapi;

declare_rule! {
    /// An ELF Perl extension does not depend on `perlapi-*`.
    ///
    /// A regular file or hardlink under `usr/lib/perl5/` or
    /// `usr/lib/<triplet>/perl5/` whose name ends in `.so` and whose File type
    /// is ELF. `Depends` or `Pre-Depends` must contain an alternative that is
    /// exactly `perlapi-` plus a version token, with an optional architecture
    /// restriction and nothing else. Udebs are skipped. Extra is empty.
    MissingDependencyOnPerlapi,
    code = "LN0398",
    tag = "missing-dependency-on-perlapi",
    default_selected = true,
    experimental = false,
}

static PERL_SO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/(?:[^/]+/)?perl5/.*\.so$").expect("regex"));
static PERLAPI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^perlapi-[-A-Za-z0-9_.]+(?:\s*\[[^\]]+\])?$").expect("regex"));

impl BinaryPackageRule for MissingDependencyOnPerlapi {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        if !has_perl_lib(files) || declares_perlapi(ctx.installation_control().fields()) {
            return;
        }
        ctx.diagnostic(Diagnostic::error(""));
    }
}

fn has_perl_lib(files: &FileTree) -> bool {
    files.entries().iter().any(|entry| {
        is_elf(entry)
            && entry
                .path()
                .to_str()
                .is_some_and(|path| PERL_SO.is_match(path))
    })
}

fn is_elf(entry: &Entry) -> bool {
    matches!(
        entry.kind(),
        crate::lint::file_tree::EntryKind::RegularFile
            | crate::lint::file_tree::EntryKind::Hardlink
    ) && matches!(entry.file_type(), Some(FileType::Elf { .. }))
}

fn declares_perlapi(fields: &ControlParagraph) -> bool {
    ["Pre-Depends", "Depends"].into_iter().any(|name| {
        fields.relation(name).is_some_and(|relation| {
            relation
                .alternatives()
                .any(|alternative| PERLAPI.is_match(alternative.text().trim()))
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(_path: &str) -> TestFile {
        TestFile::new("elf").file_type(FileType::Elf {
            has_static_symbol_table: false,
            executable: false,
        })
    }

    fn packaged(path: &str, depends: Option<&str>) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(vec![(path.to_string(), elf(path))]);
        if let Some(depends) = depends {
            case = case.field("Depends", depends);
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("usr/lib/perl5/Foo.so", Some("perlapi-5.36")),
            packaged(
                "usr/lib/x86_64-linux-gnu/perl5/auto/Foo/Foo.so",
                Some("perlapi-5.36.0 [amd64]"),
            ),
            packaged("usr/lib/perl5/Foo.so", Some("perl | perlapi-5.40")),
            packaged("usr/lib/perl5/Foo.so", Some("perlapi-5.36")).field("Pre-Depends", "perl"),
            packaged("usr/share/perl5/Foo.so", None),
            packaged("usr/lib/perl/5.36/Foo.so", None),
            packaged("usr/lib/triplet/extra/perl5/Foo.so", None),
            packaged("usr/lib/perl5/Foo.pl", None),
            BinaryPackageCase::from(vec![(
                "usr/lib/perl5/Foo.so".to_string(),
                TestFile::new("not elf"),
            )]),
            packaged("usr/lib/perl5/Foo.so", None).udeb(),
        ];
        let fail = vec![
            packaged("usr/lib/perl5/auto/Foo/Foo.so", None),
            packaged("usr/lib/perl5/Foo.so", Some("perl")),
            packaged("usr/lib/perl5/Foo.so", Some("perlapi-5.36 (>= 1)")),
            packaged("usr/lib/perl5/Foo.so", Some("${perl:Depends}")),
            packaged("usr/lib/x86_64-linux-gnu/perl5/Foo.so", Some("libfoo"))
                .field("Pre-Depends", "perlapi-5.36:any"),
        ];
        BinaryPackageTester::new(MissingDependencyOnPerlapi, pass, fail).test_and_snapshot();
    }
}
