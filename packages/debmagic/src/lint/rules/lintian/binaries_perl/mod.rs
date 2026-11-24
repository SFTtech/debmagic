//! `binaries/prerequisites/perl`: an ELF `*.so` under `perl5` needs `perlapi-*`.

mod ln0398_missing_dependency_on_perlapi;

pub(crate) use ln0398_missing_dependency_on_perlapi::MissingDependencyOnPerlapi;
