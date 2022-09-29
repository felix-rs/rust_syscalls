#![no_std]

#[cfg(all(feature = "direct", feature = "indirect"))]
compile_error!("\t [!] RUST_SYSCALLS ERROR: feature \"direct\" and feature \"indirect\" cannot be enabled at the same time");

#[cfg(not(any(feature = "direct", feature = "indirect")))]
compile_error!(
    "\t [!] RUST_SYSCALLS ERROR: feature \"direct\" or feature \"indirect\" must be enabled"
);

pub mod obf;
pub mod syscall;
pub mod syscall_resolve;
