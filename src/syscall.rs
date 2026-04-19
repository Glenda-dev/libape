use crate::fallback::invoke_linux_syscall;
use crate::mem;
use crate::policy::{FutexOpClass, classify_futex_op};
use crate::runtime::{with_runtime, with_runtime_read};
use crate::state::{ApeProcessIds, RoutePolicy};
use linux_raw_sys::errno::{EAGAIN, EINVAL, ENOSYS};
use linux_raw_sys::general::{
    __NR_brk, __NR_chdir, __NR_chroot, __NR_futex, __NR_getcwd, __NR_getegid, __NR_geteuid,
    __NR_getgid, __NR_getpid, __NR_getppid, __NR_getrandom, __NR_gettid, __NR_getuid,
    __NR_rt_sigpending, __NR_rt_sigprocmask, __NR_sched_yield, __NR_set_tid_address,
    SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    LocalFast,
    LocalSlow,
    Unsupported,
}

const DAEMON_POLL_BUDGET: usize = 8;
const SLOW_RESULT_WAIT_SPINS: usize = 64;

#[inline]
const fn errno(code: u32) -> isize {
    -(code as isize)
}

pub fn classify_syscall(sys_num: usize) -> RouteClass {
    match sys_num as u32 {
        __NR_getpid | __NR_gettid | __NR_getppid | __NR_getrandom | __NR_sched_yield
        | __NR_set_tid_address | __NR_getuid | __NR_geteuid | __NR_getgid | __NR_getegid => {
            RouteClass::LocalFast
        }
        __NR_getcwd | __NR_chdir | __NR_chroot | __NR_brk | __NR_futex | __NR_rt_sigprocmask
        | __NR_rt_sigpending => RouteClass::LocalSlow,
        _ => RouteClass::Unsupported,
    }
}

pub fn dispatch_syscall(sys_num: usize, args: [usize; 6]) -> isize {
    daemon_tick();

    let ret = match classify_syscall(sys_num) {
        RouteClass::LocalFast => {
            with_runtime(|rt| rt.mark_local_fast_hit());
            dispatch_local_fast(sys_num as u32, args)
        }
        RouteClass::LocalSlow => dispatch_local_slow(sys_num as u32, args),
        RouteClass::Unsupported => {
            with_runtime(|rt| rt.mark_unsupported_hit());
            fallback_or_errno(sys_num, args)
        }
    };

    daemon_tick();
    ret
}

fn dispatch_local_fast(sys_num: u32, args: [usize; 6]) -> isize {
    match sys_num {
        __NR_getpid => local_getpid(),
        __NR_gettid => local_gettid(),
        __NR_getppid => local_getppid(),
        __NR_getrandom => local_getrandom(args[0] as *mut u8, args[1]),
        __NR_sched_yield => 0,
        __NR_set_tid_address => local_set_tid_address(args[0]),
        __NR_getuid => local_getuid(),
        __NR_geteuid => local_geteuid(),
        __NR_getgid => local_getgid(),
        __NR_getegid => local_getegid(),
        _ => errno(ENOSYS),
    }
}

fn dispatch_local_slow(sys_num: u32, args: [usize; 6]) -> isize {
    with_runtime(|rt| rt.mark_local_slow_hit());

    let queue_id = with_runtime(|rt| {
        if !rt.slow_path_enabled() {
            return None;
        }
        rt.enqueue_slow_syscall(sys_num as usize, args)
    });

    let Some(id) = queue_id else {
        return fallback_or_errno(sys_num as usize, args);
    };

    if let Some(ret) = wait_slow_result(id) {
        return ret;
    }

    fallback_or_errno(sys_num as usize, args)
}

fn local_getpid() -> isize {
    with_runtime_read(|rt| match rt.process_ids() {
        Some(ids) => ids.pid as isize,
        None => errno(ENOSYS),
    })
}

fn current_tid() -> Option<usize> {
    let tid = invoke_linux_syscall(__NR_gettid as usize, [0; 6]);
    (tid > 0).then_some(tid as usize)
}

fn current_is_service_thread() -> bool {
    let Some(curr_tid) = current_tid() else {
        return false;
    };
    with_runtime_read(|rt| rt.service_tid() == Some(curr_tid))
}

fn daemon_tick() {
    if !current_is_service_thread() {
        return;
    }

    for _ in 0..DAEMON_POLL_BUDGET {
        if service_poll_once() == 0 {
            break;
        }
    }
}

fn wait_slow_result(id: u64) -> Option<isize> {
    if let Some(ret) = with_runtime(|rt| rt.take_slow_result(id)) {
        return Some(ret);
    }

    if current_is_service_thread() {
        for _ in 0..DAEMON_POLL_BUDGET {
            if service_poll_once() == 0 {
                break;
            }
            if let Some(ret) = with_runtime(|rt| rt.take_slow_result(id)) {
                return Some(ret);
            }
        }
        return with_runtime(|rt| rt.take_slow_result(id));
    }

    for _ in 0..SLOW_RESULT_WAIT_SPINS {
        if let Some(ret) = with_runtime(|rt| rt.take_slow_result(id)) {
            return Some(ret);
        }
        let _ = invoke_linux_syscall(__NR_sched_yield as usize, [0; 6]);
    }

    None
}

fn local_gettid() -> isize {
    with_runtime_read(|rt| match rt.process_ids() {
        Some(ids) => ids.tid as isize,
        None => errno(ENOSYS),
    })
}

fn local_getppid() -> isize {
    with_runtime_read(|rt| match rt.process_ids() {
        Some(ids) => ids.ppid as isize,
        None => errno(ENOSYS),
    })
}

fn local_set_tid_address(tid_ptr: usize) -> isize {
    with_runtime(|rt| {
        rt.set_clear_child_tid(tid_ptr);
        match rt.process_ids() {
            Some(ids) => ids.tid as isize,
            None => errno(ENOSYS),
        }
    })
}

fn local_getrandom(buf: *mut u8, len: usize) -> isize {
    match mem::write_zeroed(buf, len) {
        Ok(()) => len as isize,
        Err(e) => e,
    }
}

fn local_getuid() -> isize {
    with_runtime_read(|rt| rt.process_state().uid as isize)
}

fn local_geteuid() -> isize {
    with_runtime_read(|rt| rt.process_state().euid as isize)
}

fn local_getgid() -> isize {
    with_runtime_read(|rt| rt.process_state().gid as isize)
}

fn local_getegid() -> isize {
    with_runtime_read(|rt| rt.process_state().egid as isize)
}

fn local_getcwd(buf: *mut u8, size: usize) -> isize {
    if size == 0 {
        return errno(EINVAL);
    }

    let cwd = with_runtime_read(|rt| {
        let path = &rt.process_state().fd.cwd;
        if path.is_empty() { "/".into() } else { path.clone() }
    });

    match mem::write_cstr(buf, size, &cwd) {
        Ok(_) => buf as isize,
        Err(e) => e,
    }
}

fn local_chdir(path_ptr: *const u8) -> isize {
    let path = match mem::read_cstr(path_ptr, 4096) {
        Ok(v) => v,
        Err(e) => return e,
    };

    with_runtime(|rt| {
        rt.process_state_mut().fd.cwd = path;
    });
    0
}

fn local_chroot(path_ptr: *const u8) -> isize {
    let path = match mem::read_cstr(path_ptr, 4096) {
        Ok(v) => v,
        Err(e) => return e,
    };

    with_runtime(|rt| {
        let proc = rt.process_state_mut();
        proc.fd.root_dir = path;
        if proc.fd.cwd.is_empty() {
            proc.fd.cwd = "/".into();
        }
    });
    0
}

fn local_brk(addr: usize) -> isize {
    with_runtime(|rt| {
        let mem = &mut rt.process_state_mut().memory;
        if mem.brk_start == 0 && addr != 0 {
            mem.brk_start = addr;
            mem.brk_current = addr;
        }

        if addr == 0 {
            return mem.brk_current as isize;
        }

        if addr < mem.brk_start || addr > mem.heap_limit {
            return mem.brk_current as isize;
        }

        mem.brk_current = addr;
        mem.brk_current as isize
    })
}

fn local_futex(uaddr: usize, futex_op: usize, val: usize) -> isize {
    match classify_futex_op(futex_op) {
        FutexOpClass::Wake => {
            with_runtime(|rt| rt.process_state_mut().futex.wake_waiters(uaddr, val) as isize)
        }
        FutexOpClass::Wait => {
            with_runtime(|rt| rt.process_state_mut().futex.register_waiter(uaddr));
            errno(EAGAIN)
        }
        FutexOpClass::Other => 0,
    }
}

fn local_rt_sigprocmask(
    how: usize,
    set_ptr: *const u64,
    oldset_ptr: *mut u64,
    sigsetsize: usize,
) -> isize {
    if sigsetsize != core::mem::size_of::<u64>() {
        return errno(EINVAL);
    }

    let old_mask = with_runtime_read(|rt| rt.process_state().signal.blocked);
    if !oldset_ptr.is_null()
        && let Err(e) = mem::write_u64(oldset_ptr, old_mask)
    {
        return e;
    }

    if set_ptr.is_null() {
        return 0;
    }

    let set_mask = match mem::read_u64(set_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let ok = with_runtime(|rt| {
        let sig = &mut rt.process_state_mut().signal;
        match how as u32 {
            SIG_BLOCK => sig.blocked |= set_mask,
            SIG_UNBLOCK => sig.blocked &= !set_mask,
            SIG_SETMASK => sig.blocked = set_mask,
            _ => return false,
        }
        true
    });

    if ok { 0 } else { errno(EINVAL) }
}

fn local_rt_sigpending(set_ptr: *mut u64, sigsetsize: usize) -> isize {
    if sigsetsize != core::mem::size_of::<u64>() {
        return errno(EINVAL);
    }

    let value = with_runtime_read(|rt| {
        let sig = &rt.process_state().signal;
        sig.pending & sig.blocked
    });

    match mem::write_u64(set_ptr, value) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn process_slow_request(req: crate::state::SlowSyscallRequest) -> isize {
    match req.sys_num as u32 {
        __NR_getcwd => local_getcwd(req.args[0] as *mut u8, req.args[1]),
        __NR_chdir => local_chdir(req.args[0] as *const u8),
        __NR_chroot => local_chroot(req.args[0] as *const u8),
        __NR_brk => local_brk(req.args[0]),
        __NR_futex => local_futex(req.args[0], req.args[1], req.args[2]),
        __NR_rt_sigprocmask => local_rt_sigprocmask(
            req.args[0],
            req.args[1] as *const u64,
            req.args[2] as *mut u64,
            req.args[3],
        ),
        __NR_rt_sigpending => local_rt_sigpending(req.args[0] as *mut u64, req.args[1]),
        _ => fallback_or_errno(req.sys_num, req.args),
    }
}

fn fallback_or_errno(sys_num: usize, args: [usize; 6]) -> isize {
    let (policy, fallback_enabled) =
        with_runtime_read(|rt| (rt.route_policy(), rt.fallback_enabled()));

    if fallback_enabled && policy != RoutePolicy::LocalOnly {
        with_runtime(|rt| rt.mark_fallback_hit());
        return invoke_linux_syscall(sys_num, args);
    }

    errno(ENOSYS)
}

pub fn set_process_ids(pid: usize, ppid: usize, tid: usize) {
    with_runtime(|rt| rt.set_process_ids(ApeProcessIds { pid, ppid, tid }));
}

pub fn bootstrap_init() {
    let has_ids = with_runtime_read(|rt| rt.process_ids().is_some());
    if has_ids {
        return;
    }

    let pid = invoke_linux_syscall(__NR_getpid as usize, [0; 6]);
    let tid = invoke_linux_syscall(__NR_gettid as usize, [0; 6]);
    let ppid = invoke_linux_syscall(__NR_getppid as usize, [0; 6]);

    if pid > 0 && tid > 0 {
        set_process_ids(pid as usize, ppid.max(0) as usize, tid as usize);
        register_service_thread(tid as usize);
    }
}

pub fn inject_guard_daemon() {
    bootstrap_init();

    if let Some(tid) = current_tid() {
        register_service_thread(tid);
    }

    mark_bootstrap_ready();
}

pub fn mark_bootstrap_ready() {
    with_runtime(|rt| rt.set_bootstrap_ready(true));
}

pub fn set_identity(uid: usize, euid: usize, gid: usize, egid: usize) {
    with_runtime(|rt| rt.set_identity(uid, euid, gid, egid));
}

pub fn seed_memory(
    brk_start: usize,
    brk_current: usize,
    heap_limit: usize,
    mmap_base: usize,
    mmap_next: usize,
    mmap_limit: usize,
) {
    with_runtime(|rt| {
        rt.set_memory_seed(brk_start, brk_current, heap_limit, mmap_base, mmap_next, mmap_limit)
    });
}

pub fn set_route_policy(policy: u8, slow_enabled: bool, fallback_enabled: bool) {
    with_runtime(|rt| {
        rt.set_route_policy(RoutePolicy::from_u8(policy), slow_enabled, fallback_enabled)
    });
}

pub fn register_service_thread(tid: usize) {
    with_runtime(|rt| rt.register_service_thread(tid));
}

pub fn service_poll_once() -> usize {
    let req = with_runtime(|rt| rt.dequeue_slow_request());
    let Some(req) = req else {
        return 0;
    };

    let ret = process_slow_request(req);
    with_runtime(|rt| {
        let _ = rt.complete_slow_syscall(req.id, ret);
    });
    1
}

pub fn get_stat(field: usize) -> u64 {
    let s = with_runtime_read(|rt| rt.stats_snapshot());
    match field {
        0 => s.local_fast_hits,
        1 => s.local_slow_hits,
        2 => s.slow_enqueued,
        3 => s.fallback_hits,
        4 => s.unsupported_hits,
        5 => s.queue_drops,
        _ => 0,
    }
}
