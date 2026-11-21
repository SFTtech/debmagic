use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct MagicArchInArchList;

declare_rule! {
    /// The `any` architecture is mixed with other values in `Architecture`.
    ///
    /// Only for a list of two or more values. The list is the Dsc
    /// `Architecture` of a Source package, or the Source paragraph's
    /// `Architecture` of a Source tree. When `any` is in the list, every other
    /// value except `all` must be absent: `any amd64` fails, `any all` passes,
    /// and `any any` passes. A source's `all` is never flagged. Extra is
    /// empty and there is no Location. Applies to the Source tree and Source
    /// package.
    MagicArchInArchList,
    code = "LN0901",
    tag = "magic-arch-in-arch-list",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MagicArchInArchList {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let architectures: Vec<String> = match ctx.dsc() {
            Some(dsc) => split_list(dsc.fields().get("Architecture")),
            None => split_list(
                ctx.debian_control()
                    .and_then(|control| control.source().get("Architecture")),
            ),
        };
        if architectures.len() > 1 && mixes_any(&architectures) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

fn split_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Lintian's `magic_error` for a source: `any` present, and something other
/// than `any` or `all` alongside it. A source's `all` is allowed.
fn mixes_any(architectures: &[String]) -> bool {
    if !architectures
        .iter()
        .any(|architecture| architecture == "any")
    {
        return false;
    }
    architectures
        .iter()
        .any(|architecture| architecture != "any" && architecture != "all")
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    fn dsc(architecture: &str) -> SourcePackageCase {
        let files = vec![("debian/control".to_string(), TestFile::new(CONTROL))];
        SourcePackageCase::from(files).dsc(
            Dsc::parse(&format!(
                "Format: 3.0 (native)\nSource: example\nVersion: 1.0\nArchitecture: {architecture}\n"
            ))
            .expect("dsc"),
        )
    }

    #[test]
    fn source_package() {
        let pass = vec![
            dsc("any"),
            dsc("all"),
            dsc("any all"),
            dsc("any any"),
            dsc("amd64 arm64"),
            dsc("all amd64"),
        ];
        let fail = vec![dsc("any amd64"), dsc("any all i386"), dsc("any i386 amd64")];
        SourcePackageTester::new(MagicArchInArchList, pass, fail).test_and_snapshot();
    }
}
