use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct RequiresRApi;

declare_rule! {
    /// A binary package ships an R site library, a `DESCRIPTION` file under
    /// `usr/lib/R/site-library/`, but no `Pre-Depends` or `Depends` clause is
    /// exactly one `r-api-<version>` package without a version constraint.
    ///
    /// Any depth below the site library counts, and a directory named
    /// `DESCRIPTION` does not. A clause with a version, an architecture, or an
    /// alternative does not satisfy the prerequisite. Extra is empty. Udebs are
    /// checked too.
    RequiresRApi,
    code = "LN1406",
    tag = "requires-r-api",
    default_selected = true,
    experimental = false,
}

const SITE_LIBRARY: &[u8] = b"usr/lib/R/site-library/";
const DESCRIPTION: &[u8] = b"/DESCRIPTION";

impl BinaryPackageRule for RequiresRApi {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let ships_site_library = files.entries().iter().any(|entry| {
            let bytes = entry.path().as_os_str().as_encoded_bytes();
            entry.kind() != EntryKind::Directory
                && bytes.starts_with(SITE_LIBRARY)
                && bytes.ends_with(DESCRIPTION)
                && bytes.len() >= SITE_LIBRARY.len() + DESCRIPTION.len()
        });
        if !ships_site_library {
            return;
        }
        let strong = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"]);
        let has_r_api = strong
            .dependencies()
            .iter()
            .any(|clause| clause.len() == 1 && is_bare_r_api(clause[0].text()));
        if !has_r_api {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

/// `^r-api-[\w\d+-.]+$`: the predicate is the whole clause text, so a version,
/// an architecture, or a profile does not match.
fn is_bare_r_api(text: &str) -> bool {
    text.strip_prefix("r-api-").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || matches!(c, '+' | ',' | '-' | '.'))
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn library(depends: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/R/site-library/foo/DESCRIPTION".to_string(),
            TestFile::new("Package: foo\n"),
        )])
        .field("Depends", depends)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // A directory named DESCRIPTION is not the file.
            BinaryPackageCase::from(vec![(
                "usr/lib/R/site-library/foo/DESCRIPTION".to_string(),
                TestFile::directory(),
            )])
            .field("Depends", "libc6"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Depends", "libc6"),
            library("r-api-4, libc6"),
            library("libc6, r-api-4.0"),
            BinaryPackageCase::from(vec![(
                "usr/lib/R/site-library/foo/DESCRIPTION".to_string(),
                TestFile::new("x"),
            )])
            .field("Pre-Depends", "r-api-4"),
        ];
        let fail = vec![
            library("libc6"),
            // A version constraint does not count.
            library("r-api-4 (>= 1)"),
            // An alternative does not count.
            library("r-api-4 | libc6"),
            // Recommends is not strong.
            BinaryPackageCase::from(vec![(
                "usr/lib/R/site-library/foo/DESCRIPTION".to_string(),
                TestFile::new("x"),
            )])
            .field("Recommends", "r-api-4"),
        ];
        BinaryPackageTester::new(RequiresRApi, pass, fail).test_and_snapshot();
    }
}
