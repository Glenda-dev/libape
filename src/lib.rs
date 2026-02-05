#![no_std]
#![allow(dead_code)]
#![allow(unused)]

extern crate glenda;

#[unsafe(no_mangle)]
pub extern "C" fn __glenda_syscall_dispatch(
    n: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
    a5: isize,
    a6: isize,
) -> isize {
    match n {
        _ => -38, // -ENOSYS
    }
}
