//! Rules for lintian's `fields/checksums` Check.

mod ln1049_no_strong_digests_in_dsc;

pub(crate) use ln1049_no_strong_digests_in_dsc::NoStrongDigestsInDsc;
