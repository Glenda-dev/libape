#![no_std]
#![allow(dead_code)]
#![allow(unused)]

extern crate alloc;

mod ape;
mod metadata;
mod syscall;

use crate::ape::ApeService;
use glenda::interface::linux::*;
use glenda::protocol::linux::*; // Imports all Linux*Service traits

static SERVICE: ApeService = ApeService::new();

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
    let n = n as usize;
    let u1 = a1 as usize;
    let u2 = a2 as usize;
    let u3 = a3 as usize;
    let u4 = a4 as usize;
    let u5 = a5 as usize;
    let u6 = a6 as usize;

    match n {
        SYS_GETCWD => SERVICE.sys_getcwd(u1 as *mut u8, u2),
        SYS_DUP => SERVICE.sys_dup(u1),
        SYS_DUP3 => SERVICE.sys_dup3(u1, u2, u3),
        SYS_MKDIRAT => SERVICE.sys_mkdirat(u1, u2 as *const u8, u3),
        SYS_UNLINKAT => SERVICE.sys_unlinkat(u1, u2 as *const u8, u3),
        SYS_CHDIR => SERVICE.sys_chdir(u1 as *const u8),
        SYS_OPENAT => SERVICE.sys_openat(u1, u2 as *const u8, u3, u4),
        SYS_CLOSE => SERVICE.sys_close(u1),
        SYS_PIPE2 => SERVICE.sys_pipe2(u1 as *mut i32, u2),
        SYS_GETDENTS64 => SERVICE.sys_getdents64(u1, u2 as *mut u8, u3),
        SYS_LSEEK => SERVICE.sys_lseek(u1, a2, u3), // lseek offset is signed
        SYS_READ => SERVICE.sys_read(u1, u2 as *mut u8, u3),
        SYS_WRITE => SERVICE.sys_write(u1, u2 as *const u8, u3),
        SYS_READLINKAT => SERVICE.sys_readlinkat(u1, u2 as *const u8, u3 as *mut u8, u4),
        SYS_NEWFSTATAT => SERVICE.sys_newfstatat(u1, u2 as *const u8, u3 as *mut u8, u4),
        SYS_FSTAT => SERVICE.sys_fstat(u1, u2 as *mut u8),
        SYS_EXIT => SERVICE.sys_exit(u1),
        SYS_EXIT_GROUP => SERVICE.sys_exit_group(u1),
        SYS_CLOCK_GETTIME => SERVICE.sys_clock_gettime(u1, u2 as *mut u8),
        SYS_KILL => SERVICE.sys_kill(u1, u2),
        SYS_UNAME => SERVICE.sys_uname(u1 as *mut u8),
        SYS_GETPID => SERVICE.sys_getpid(),
        SYS_GETPPID => SERVICE.sys_getppid(),
        SYS_GETUID => SERVICE.sys_getuid(),
        SYS_GETEUID => SERVICE.sys_geteuid(),
        SYS_GETGID => SERVICE.sys_getgid(),
        SYS_GETEGID => SERVICE.sys_getegid(),
        SYS_GETTID => SERVICE.sys_gettid(),
        SYS_BRK => SERVICE.sys_brk(u1),
        SYS_MUNMAP => SERVICE.sys_munmap(u1, u2),
        SYS_CLONE => SERVICE.sys_clone(u1, u2, u3 as *mut u32, u4, u5 as *mut u32),
        SYS_EXECVE => {
            SERVICE.sys_execve(u1 as *const u8, u2 as *const *const u8, u3 as *const *const u8)
        }
        SYS_MMAP => SERVICE.sys_mmap(u1, u2, u3, u4, u5, u6),
        SYS_MPROTECT => SERVICE.sys_mprotect(u1, u2, u3),
        SYS_WAIT4 => SERVICE.sys_wait4(a1, u2 as *mut i32, u3, u4 as *mut u8),
        SYS_PRLIMIT64 => SERVICE.sys_prlimit64(u1, u2, u3 as *const u8, u4 as *mut u8),
        _ => -ENOSYS,
    }
}
