//! Rules for lintian's `fields/mail-address` Check.
//!
//! Mailboxes come from Maintainer and Uploaders on the Source package or Source
//! tree, and from Maintainer on a Binary package's Installation control.
//! Each Rule emits one Tag.

mod ln0370_mail_address_loops_or_bounces;
mod ln0371_faulty_debian_qa_group_address;
mod ln0372_faulty_debian_qa_group_phrase;
mod mailbox;

pub(crate) use ln0370_mail_address_loops_or_bounces::MailAddressLoopsOrBounces;
pub(crate) use ln0371_faulty_debian_qa_group_address::FaultyDebianQaGroupAddress;
pub(crate) use ln0372_faulty_debian_qa_group_phrase::FaultyDebianQaGroupPhrase;
