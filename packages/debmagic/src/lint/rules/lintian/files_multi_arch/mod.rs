//! Rules for lintian's `files/multi-arch` Check.

mod ln0721_multiarch_foreign_pkgconfig;

pub(crate) use ln0721_multiarch_foreign_pkgconfig::MultiarchForeignPkgconfig;
