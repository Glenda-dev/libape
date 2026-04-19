use crate::state::{ApeProcessIds, ApeProcessState, RoutePolicy, RouteStats, SlowSyscallRequest};
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinMutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinMutex<T> {}

pub struct SpinMutexGuard<'a, T> {
    lock: &'a SpinMutex<T>,
}

impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        SpinMutexGuard { lock: self }
    }
}

impl<T> Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
pub struct ApeRuntime {
    process: ApeProcessState,
    initialized: bool,
    bootstrap_ready: bool,
}

impl ApeRuntime {
    pub fn uninitialized() -> Self {
        Self { process: ApeProcessState::new(), initialized: false, bootstrap_ready: false }
    }

    pub fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn set_bootstrap_ready(&mut self, ready: bool) {
        self.bootstrap_ready = ready;
    }

    pub fn bootstrap_ready(&self) -> bool {
        self.bootstrap_ready
    }

    pub fn set_process_ids(&mut self, ids: ApeProcessIds) {
        self.process.set_ids(ids);
    }

    pub fn process_ids(&self) -> Option<ApeProcessIds> {
        self.process.ids()
    }

    pub fn set_clear_child_tid(&mut self, ptr: usize) {
        self.process.signal.clear_child_tid = ptr;
    }

    pub fn set_identity(&mut self, uid: usize, euid: usize, gid: usize, egid: usize) {
        self.process.set_identity(uid, euid, gid, egid);
    }

    pub fn set_memory_seed(
        &mut self,
        brk_start: usize,
        brk_current: usize,
        heap_limit: usize,
        mmap_base: usize,
        mmap_next: usize,
        mmap_limit: usize,
    ) {
        self.process.memory.brk_start = brk_start;
        self.process.memory.brk_current = brk_current;
        self.process.memory.heap_limit = heap_limit;
        self.process.memory.mmap_base = mmap_base;
        self.process.memory.mmap_next = mmap_next;
        self.process.memory.mmap_limit = mmap_limit;
    }

    pub fn set_route_policy(&mut self, policy: RoutePolicy, slow_enabled: bool, fallback: bool) {
        self.process.set_route_config(policy, slow_enabled, fallback);
    }

    pub fn route_policy(&self) -> RoutePolicy {
        self.process.route_config.policy
    }

    pub fn slow_path_enabled(&self) -> bool {
        self.process.route_config.slow_path_enabled
    }

    pub fn fallback_enabled(&self) -> bool {
        self.process.route_config.fallback_enabled
    }

    pub fn register_service_thread(&mut self, tid: usize) {
        self.process.service_tid = Some(tid);
    }

    pub fn service_tid(&self) -> Option<usize> {
        self.process.service_tid
    }

    pub fn enqueue_slow_syscall(&mut self, sys_num: usize, args: [usize; 6]) -> Option<u64> {
        let id = self.process.slow_queue.enqueue_request(sys_num, args);
        if id.is_some() {
            self.process.route_stats.slow_enqueued =
                self.process.route_stats.slow_enqueued.saturating_add(1);
        } else {
            self.process.route_stats.queue_drops =
                self.process.route_stats.queue_drops.saturating_add(1);
        }
        id
    }

    pub fn dequeue_slow_request(&mut self) -> Option<SlowSyscallRequest> {
        self.process.slow_queue.dequeue_request()
    }

    pub fn complete_slow_syscall(&mut self, id: u64, ret: isize) -> bool {
        self.process.slow_queue.push_result(id, ret)
    }

    pub fn take_slow_result(&mut self, id: u64) -> Option<isize> {
        self.process.slow_queue.take_result(id)
    }

    pub fn pending_slow_len(&self) -> usize {
        self.process.slow_queue.pending_len()
    }

    pub fn stats_snapshot(&self) -> RouteStats {
        self.process.route_stats
    }

    pub fn mark_local_fast_hit(&mut self) {
        self.process.route_stats.local_fast_hits =
            self.process.route_stats.local_fast_hits.saturating_add(1);
    }

    pub fn mark_local_slow_hit(&mut self) {
        self.process.route_stats.local_slow_hits =
            self.process.route_stats.local_slow_hits.saturating_add(1);
    }

    pub fn mark_fallback_hit(&mut self) {
        self.process.route_stats.fallback_hits =
            self.process.route_stats.fallback_hits.saturating_add(1);
    }

    pub fn mark_unsupported_hit(&mut self) {
        self.process.route_stats.unsupported_hits =
            self.process.route_stats.unsupported_hits.saturating_add(1);
    }

    pub fn process_state(&self) -> &ApeProcessState {
        &self.process
    }

    pub fn process_state_mut(&mut self) -> &mut ApeProcessState {
        &mut self.process
    }
}

static RUNTIME: SpinMutex<Option<ApeRuntime>> = SpinMutex::new(None);

pub fn init_runtime() {
    let mut guard = RUNTIME.lock();
    if guard.is_none() {
        let mut rt = ApeRuntime::uninitialized();
        rt.mark_initialized();
        *guard = Some(rt);
        return;
    }

    if let Some(rt) = guard.as_mut() {
        rt.mark_initialized();
    }
}

pub fn with_runtime<R>(f: impl FnOnce(&mut ApeRuntime) -> R) -> R {
    init_runtime();
    let mut guard = RUNTIME.lock();
    let runtime = match guard.as_mut() {
        Some(rt) => rt,
        None => panic!("libape runtime not initialized"),
    };
    f(runtime)
}

pub fn with_runtime_read<R>(f: impl FnOnce(&ApeRuntime) -> R) -> R {
    init_runtime();
    let guard = RUNTIME.lock();
    let runtime = match guard.as_ref() {
        Some(rt) => rt,
        None => panic!("libape runtime not initialized"),
    };
    f(runtime)
}
