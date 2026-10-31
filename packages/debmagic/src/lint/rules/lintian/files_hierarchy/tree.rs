use crate::lint::tester::{BinaryPackageCase, TestFile};

/// A Data files fixture. `true` is a directory; `false` is a regular file `"x"`.
pub(super) fn tree(entries: &[(&str, bool)]) -> BinaryPackageCase {
    BinaryPackageCase::from(
        entries
            .iter()
            .map(|(path, dir)| {
                let file = if *dir {
                    TestFile::directory()
                } else {
                    TestFile::new("x")
                };
                (path.to_string(), file)
            })
            .collect::<Vec<_>>(),
    )
}
