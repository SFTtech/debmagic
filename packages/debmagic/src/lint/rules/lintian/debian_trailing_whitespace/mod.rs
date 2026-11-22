//! Rules for lintian's `debian/trailing-whitespace` Check.

mod ln0830_no_newline_at_end;
mod ln1336_trailing_whitespace;

pub(crate) use ln0830_no_newline_at_end::NoNewlineAtEnd;
pub(crate) use ln1336_trailing_whitespace::TrailingWhitespace;
