//! Rules for lintian's `scripts` Check that match commands in maintainer scripts.
//!
//! `script` walks those Control files. The dependency Tags read `Depends` and
//! `Pre-Depends` with `ControlParagraph::relation` and treat a package name as
//! present; they do not implement `satisfies` or compare versions. Each Rule
//! owns one Tag.

mod ln0163_maintainer_script_needs_depends_on_adduser;
mod ln0164_maintainer_script_needs_depends_on_update_inetd;
mod ln0165_maintainer_script_calls_gconftool;
mod ln0166_maintainer_script_calls_start_stop_daemon;
mod script;

pub(crate) use ln0163_maintainer_script_needs_depends_on_adduser::MaintainerScriptNeedsDependsOnAdduser;
pub(crate) use ln0164_maintainer_script_needs_depends_on_update_inetd::MaintainerScriptNeedsDependsOnUpdateInetd;
pub(crate) use ln0165_maintainer_script_calls_gconftool::MaintainerScriptCallsGconftool;
pub(crate) use ln0166_maintainer_script_calls_start_stop_daemon::MaintainerScriptCallsStartStopDaemon;
