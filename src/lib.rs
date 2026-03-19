#![no_std]

//! libape: Linux syscall interface for Glenda

extern crate alloc;

pub mod ape;
pub mod cap;
pub mod client;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod sys;
pub mod vdso;
pub mod version;
