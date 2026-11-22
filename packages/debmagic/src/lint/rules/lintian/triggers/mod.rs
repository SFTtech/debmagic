//! `triggers` Check.
//!
//! Each Rule owns one Tag. The control member is the `triggers` file.

mod ln0355_uses_implicit_await_trigger;
mod ln1231_repeated_trigger_name;

pub(crate) use ln0355_uses_implicit_await_trigger::UsesImplicitAwaitTrigger;
pub(crate) use ln1231_repeated_trigger_name::RepeatedTriggerName;
