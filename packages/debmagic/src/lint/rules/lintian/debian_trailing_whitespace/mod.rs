//! Rules for lintian's `debian/trailing-whitespace` Check.

mod ln0830_no_newline_at_end;

pub(crate) use ln0830_no_newline_at_end::NoNewlineAtEnd;
