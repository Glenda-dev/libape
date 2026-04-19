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
mod runtime;
mod state;
mod syscall;
pub mod version;

#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
pub extern "C" fn __libape_init() {
    runtime::init_runtime();
    syscall::inject_guard_daemon();
}

#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_inject_guard_daemon() {
    runtime::init_runtime();
    syscall::inject_guard_daemon();
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
///
/// 该符号保持 C ABI，便于后续在启动桩或服务线程中直接调用。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_ids(pid: usize, ppid: usize, tid: usize) {
    runtime::init_runtime();
    syscall::set_process_ids(pid, ppid, tid);
}

/// 标记 libape 运行时已完成注入握手（Phase-1 占位，Phase-2 将接入线程握手流程）。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_mark_ready() {
    runtime::init_runtime();
    syscall::mark_bootstrap_ready();
}

/// 设置本地身份信息（Phase-4：uid/gid 语义下沉）。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_identity(uid: usize, euid: usize, gid: usize, egid: usize) {
    runtime::init_runtime();
    syscall::set_identity(uid, euid, gid, egid);
}

/// 注入初始内存布局（Phase-2/4：execve 后的 seed 数据）。
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

/// 路由策略控制（Phase-6）：
/// policy: 0=LocalOnly, 1=PreferLocal, 2=PreferFallback
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_set_route_policy(
    policy: u8,
    slow_path_enabled: bool,
    fallback_enabled: bool,
) {
    runtime::init_runtime();
    syscall::set_route_policy(policy, slow_path_enabled, fallback_enabled);
}

/// 注册慢路径服务线程 tid（Phase-2 混合线程）。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_register_service_thread(tid: usize) {
    runtime::init_runtime();
    syscall::register_service_thread(tid);
}

/// 慢路径服务线程轮询执行一次，返回处理条目数。
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_service_poll_once() -> usize {
    runtime::init_runtime();
    syscall::service_poll_once()
}

/// 导出统计信息（Phase-7 观测）。
/// fields:
/// 0 local_fast_hits
/// 1 local_slow_hits
/// 2 slow_enqueued
/// 3 fallback_hits
/// 4 unsupported_hits
/// 5 queue_drops
#[unsafe(no_mangle)]
pub extern "C" fn __libape_runtime_get_stat(field: usize) -> u64 {
    runtime::init_runtime();
    syscall::get_stat(field)
}
