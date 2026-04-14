#![no_std]
#![no_main]

//! libape: Linux syscall interface for Glenda
#[macro_use]
extern crate glenda;
extern crate alloc;

#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub extern "C" fn __libape_init() {}

#[unsafe(no_mangle)]
pub extern "C" fn __libape_syscall_dispatch(
    n: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
) -> isize {
    -(linux_raw_sys::errno::ENOSYS as isize)
}
