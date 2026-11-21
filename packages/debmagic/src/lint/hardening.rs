//! Hardening build flags lintian recommends per architecture, and the
//! toolchain test `Binaries::Static` and `Binaries::Hardening` share.

use std::collections::HashMap;
use std::sync::LazyLock;

use std::path::PathBuf;

use super::context::BinaryPackageContext;
use super::elf::{ElfFacts, ElfObjectType};

/// `data/buildflags/hardening.json`: which hardening features
/// `dpkg-buildflags --query-features hardening` enables per architecture
/// with `hardening=+all`. Refreshed from dpkg by lintian, vendored here.
static RECOMMENDED_FEATURES: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| {
    #[derive(serde::Deserialize)]
    struct Table {
        recommended_features: HashMap<String, Vec<String>>,
    }
    serde_json::from_str::<Table>(include_str!("data/buildflags/hardening.json"))
        .expect("vendored hardening.json is valid")
        .recommended_features
});

/// Hardening features (`bindnow`, `fortify`, `pie`, `relro`, ...) dpkg
/// enables on `architecture` with `hardening=+all`. Empty for `all` and for
/// an architecture the table does not list.
pub(crate) fn recommended_features(architecture: &str) -> &'static [String] {
    if architecture == "all" {
        return &[];
    }
    RECOMMENDED_FEATURES
        .get(architecture)
        .map_or(&[], Vec::as_slice)
}

/// A Binary package's recommended features, from its `Architecture` field.
pub(crate) fn recommended_features_of(ctx: &BinaryPackageContext<'_>) -> &'static [String] {
    ctx.installation_control()
        .fields()
        .get("Architecture")
        .map_or(&[], recommended_features)
}

/// Data files ELF Entries that lack a hardening feature: the executables and
/// shared objects of a Binary package that are dynamically linked (have a
/// `NEEDED`), when dpkg recommends `feature` on the package's architecture,
/// the package was not built with Go, and `lacks` holds. Nothing for a udeb.
/// This is the shared body of lintian's `binaries/hardening` relro, bindnow,
/// and pie hints. `None` after a global File tree failure.
pub(crate) fn lacking_hardening(
    ctx: &mut BinaryPackageContext<'_>,
    feature: &str,
    lacks: impl Fn(&ElfFacts) -> bool,
) -> Option<Vec<PathBuf>> {
    if ctx.is_udeb()
        || !recommended_features_of(ctx).iter().any(|f| f == feature)
        || built_with_golang(ctx)
    {
        return Some(Vec::new());
    }
    let elf_files = ctx.data_elf_files()?;
    Some(
        elf_files
            .iter()
            .filter(|(_, facts)| {
                matches!(
                    facts.object_type(),
                    ElfObjectType::Executable | ElfObjectType::SharedObject
                ) && !facts.needed().is_empty()
                    && lacks(facts)
            })
            .map(|(path, _)| path.to_path_buf())
            .collect(),
    )
}

/// Lintian's `built_with_golang` without the source package's
/// `Build-Depends-All`, which a Binary package Subject does not have: the
/// source or package name starts with `golang-`, `Section: golang`, or
/// `Built-Using` / `Static-Built-Using` names `golang-N.N`.
pub(crate) fn built_with_golang(ctx: &BinaryPackageContext<'_>) -> bool {
    static GOLANG: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"golang-\d\.\d+").expect("golang regex"));
    let fields = ctx.installation_control().fields();
    ctx.source_name().starts_with("golang-")
        || fields.get("Section") == Some("golang")
        || ["Built-Using", "Static-Built-Using"]
            .into_iter()
            .any(|field| {
                fields
                    .get(field)
                    .is_some_and(|value| GOLANG.is_match(value))
            })
        || ctx.package_name().starts_with("golang-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amd64_recommends_the_dpkg_hardening_features() {
        let features = recommended_features("amd64");
        for feature in ["bindnow", "fortify", "relro", "pie"] {
            assert!(features.iter().any(|f| f == feature), "{feature}");
        }
    }

    #[test]
    fn all_and_unknown_architectures_recommend_nothing() {
        assert!(recommended_features("all").is_empty());
        assert!(recommended_features("no-such-arch").is_empty());
    }
}
