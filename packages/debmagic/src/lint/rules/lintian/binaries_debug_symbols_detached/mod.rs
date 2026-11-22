//! Rules for lintian's `binaries/debug-symbols/detached` Check.

mod ln1008_debug_symbols_directly_in_usr_lib_debug;
mod ln1009_debug_symbols_not_detached;
mod ln1293_debug_file_with_no_debug_symbols;

pub(crate) use ln1008_debug_symbols_directly_in_usr_lib_debug::DebugSymbolsDirectlyInUsrLibDebug;
pub(crate) use ln1009_debug_symbols_not_detached::DebugSymbolsNotDetached;
pub(crate) use ln1293_debug_file_with_no_debug_symbols::DebugFileWithNoDebugSymbols;
