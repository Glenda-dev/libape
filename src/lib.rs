#![no_std]
#![no_main]
#![allow(non_upper_case_globals)]
#![allow(unused)]

//! libape: Linux syscall interface for Glenda
#[macro_use]
extern crate glenda;
extern crate alloc;

pub mod compat;
mod fallback;
mod mem;
pub mod path;
pub mod policy;
pub mod runtime;
pub mod state;
pub mod syscall;
pub mod version;

#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub extern "C" fn __libape_init() {
    runtime::init_runtime();
}

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
    syscall::dispatch_syscall(n, [a, b, c, d, e, f])
}

/// 由注入路径（例如 APE 在 execve 后的引导阶段）写入本地 pid/ppid/tid。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_ids(pid: usize, ppid: usize, tid: usize) {
    runtime::init_runtime();
    syscall::set_process_ids(state::ApeProcessIds { pid, ppid, tid });
}

/// 标记 libape 运行时已完成注入握手。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_mark_ready() {
    runtime::init_runtime();
    syscall::mark_bootstrap_ready();
}

/// 设置本地身份信息。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_identity(uid: usize, euid: usize, gid: usize, egid: usize) {
    runtime::init_runtime();
    syscall::set_identity(uid, euid, gid, egid);
}

/// 注入初始内存布局。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_seed_memory(
    brk_start: usize,
    brk_current: usize,
    heap_limit: usize,
    mmap_base: usize,
    mmap_next: usize,
    mmap_limit: usize,
) {
    runtime::init_runtime();
    syscall::seed_memory(brk_start, brk_current, heap_limit, mmap_base, mmap_next, mmap_limit);
}

/// 设置 APE 子系统的 Endpoint Capability。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_ape_endpoint(ep_ptr: usize) {
    runtime::init_runtime();
    syscall::set_ape_endpoint(ep_ptr);
}
