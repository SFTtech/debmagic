//! Rules for lintian's `files/ownership` Check.

mod ln1794_wrong_file_owner_uid_or_gid;

pub(crate) use ln1794_wrong_file_owner_uid_or_gid::WrongFileOwnerUidOrGid;
