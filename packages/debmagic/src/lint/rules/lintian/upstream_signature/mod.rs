//! Rules for lintian's `upstream-signature` Check.

mod ln1144_concatenated_upstream_signatures;
mod ln1145_doubly_armored_upstream_signature;
mod ln1146_explicitly_armored_upstream_signature;

pub(crate) use ln1144_concatenated_upstream_signatures::ConcatenatedUpstreamSignatures;
pub(crate) use ln1145_doubly_armored_upstream_signature::DoublyArmoredUpstreamSignature;
pub(crate) use ln1146_explicitly_armored_upstream_signature::ExplicitlyArmoredUpstreamSignature;
