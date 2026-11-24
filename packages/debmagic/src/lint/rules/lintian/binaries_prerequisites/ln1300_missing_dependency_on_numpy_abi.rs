use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::strings::printable_strings;

use super::shared::{objects, strong};

pub struct MissingDependencyOnNumpyAbi;

declare_rule! {
    /// A Python extension built against the NumPy C ABI does not depend on
    /// the `python3-numpy-abi<N>` (or `python3-numpy2-abi<N>`) virtual package.
    ///
    /// An ELF executable or shared object named as a Python 2 extension
    /// (`usr/lib/python2.<N>/` or `usr/lib/pyshared/python2.<N>/`, not ending in
    /// `_d.so`) or as a Python 3 extension (`usr/lib/python3[.<N>]/*.cpython-*-*.so`)
    /// uses the NumPy C ABI when its strings contain `numpy` and the NumPy ABI
    /// mismatch message `module compiled against ABI version %x but this version
    /// of numpy is %x`. Its strings are the printable runs of four or more
    /// characters, as `strings(1)` finds them. The Rule then reports unless some
    /// clause of `Pre-Depends` or `Depends` is a single `python3-numpy[2]-abi<N>`
    /// package, or the clause text is `python3-numpy2-abi0 | python3-numpy-abi9`
    /// or starts with `python3-numpy2-abi0 `. Alternatives in one clause are
    /// matched as the whole clause text, as lintian does. Skipped for udebs and
    /// for the `python-numpy` and `python3-numpy` packages. The extra is empty
    /// and there is no Location.
    MissingDependencyOnNumpyAbi,
    code = "LN1300",
    tag = "missing-dependency-on-numpy-abi",
    default_selected = true,
    experimental = false,
}

static PY2_EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/(?:pyshared/)?python2\.\d+/.*").expect("valid regex"));

static PY3_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python3(?:\.\d+)?/\S+\.cpython-\d+-\S+\.so$").expect("valid regex")
});

static NUMPY_MESSAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"module compiled against ABI version (?:0x)?%x but this version of numpy is (?:0x)?%x",
    )
    .expect("valid regex")
});

static NUMPY_ABI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python3-numpy2?-abi\d+$").expect("valid regex"));

static NUMPY_ABI_OR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^python3-numpy2-abi0 | python3-numpy-abi9$").expect("valid regex")
});

static NUMPY_SELF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python3?-numpy$").expect("valid regex"));

impl BinaryPackageRule for MissingDependencyOnNumpyAbi {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || NUMPY_SELF.is_match(ctx.package_name()) {
            return;
        }
        if !uses_numpy_c_abi(ctx) {
            return;
        }
        if has_numpy_abi_dependency(&strong(ctx)) {
            return;
        }
        ctx.diagnostic(Diagnostic::error(""));
    }
}

/// Whether an ELF Python extension (of the two lintian patterns) has the
/// NumPy C ABI mismatch message among its strings.
fn uses_numpy_c_abi(ctx: &mut BinaryPackageContext<'_>) -> bool {
    let candidates: Vec<PathBuf> = objects(ctx)
        .into_iter()
        .map(|object| object.path)
        .filter(|path| {
            let name = path.to_string_lossy();
            let py2 =
                PY2_EXTENSION.is_match(&name) && name.ends_with(".so") && !name.ends_with("_d.so");
            py2 || PY3_EXTENSION.is_match(&name)
        })
        .collect();
    let Some(files) = ctx.data_files() else {
        return false;
    };
    candidates.iter().any(|path| {
        let Some(entry) = files.get(path) else {
            return false;
        };
        let Ok(bytes) = files.read(entry) else {
            return false;
        };
        let strings = printable_strings(&bytes).join("\n");
        strings.contains("numpy") && NUMPY_MESSAGE.is_match(&strings)
    })
}

/// lintian's `matches` with `VISIT_OR_CLAUSE_FULL`: a clause with one
/// alternative is matched by its package name, and a clause with several by
/// its text, alternatives joined with ` | `.
fn has_numpy_abi_dependency(depends: &PackageRelation) -> bool {
    depends.dependencies().iter().any(|clause| {
        let text = match clause.as_slice() {
            [alternative] => alternative.package().to_string(),
            alternatives => alternatives
                .iter()
                .map(|alternative| {
                    alternative
                        .text()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join(" | "),
        };
        NUMPY_ABI.is_match(&text) || NUMPY_ABI_OR.is_match(&text)
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const MESSAGE: &str = "module compiled against ABI version %x but this version of numpy is %x";

    fn extension(message: &str) -> TestFile {
        let mut bytes = TestElf::shared_object().build();
        bytes.extend_from_slice(b"\0");
        bytes.extend_from_slice(message.as_bytes());
        bytes.extend_from_slice(b"\0");
        TestFile::from_bytes(bytes)
    }

    fn case(depends: Option<&str>, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(files)
            .package("python3-example")
            .field("Architecture", "amd64");
        if let Some(depends) = depends {
            case = case.field("Depends", depends);
        }
        case
    }

    const PY3: &str = "usr/lib/python3/dist-packages/example.cpython-312-x86_64-linux-gnu.so";

    #[test]
    fn binary() {
        let pass = vec![
            case(Some("python3-numpy-abi9"), vec![(PY3, extension(MESSAGE))]),
            case(
                Some("python3-numpy2-abi0 | python3-numpy-abi9"),
                vec![(PY3, extension(MESSAGE))],
            ),
            case(
                Some("python3-numpy2-abi1 (= 1)"),
                vec![(PY3, extension(MESSAGE))],
            ),
            // Not a Python extension, or no NumPy message.
            case(None, vec![("usr/lib/libfoo.so.1", extension(MESSAGE))]),
            case(None, vec![(PY3, extension("numpy only"))]),
            case(
                None,
                vec![(PY3, TestFile::from_bytes(TestElf::shared_object().build()))],
            ),
            // A Python 2 debug extension is excluded.
            case(
                None,
                vec![(
                    "usr/lib/python2.7/dist-packages/example_d.so",
                    extension(MESSAGE),
                )],
            ),
            // A udeb is not checked.
            case(None, vec![(PY3, extension(MESSAGE))]).udeb(),
            // Packages named after numpy are not checked.
            BinaryPackageCase::from(vec![(PY3, extension(MESSAGE))])
                .package("python3-numpy")
                .field("Architecture", "amd64"),
        ];
        let fail = vec![
            case(None, vec![(PY3, extension(MESSAGE))]),
            case(
                Some("python3-numpy-abi9 | python3-foo"),
                vec![(PY3, extension(MESSAGE))],
            ),
            case(
                Some("python3-foo"),
                vec![(
                    "usr/lib/python2.7/dist-packages/example.so",
                    extension(MESSAGE),
                )],
            ),
        ];
        BinaryPackageTester::new(MissingDependencyOnNumpyAbi, pass, fail).test_and_snapshot();
    }
}
