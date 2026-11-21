//! Rules for lintian's `binaries/debug-symbols/detached` Check.

mod ln1008_debug_symbols_directly_in_usr_lib_debug;
mod ln1009_debug_symbols_not_detached;

pub(crate) use ln1008_debug_symbols_directly_in_usr_lib_debug::DebugSymbolsDirectlyInUsrLibDebug;
pub(crate) use ln1009_debug_symbols_not_detached::DebugSymbolsNotDetached;
