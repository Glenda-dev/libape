#![no_std]
#![no_main]

//! libape: Linux syscall interface for Glenda
#[macro_use]
extern crate glenda;
extern crate alloc;

pub mod ape;
pub mod arch;
pub mod cap;
pub mod client;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod sys;
pub mod vdso;
pub mod version;
