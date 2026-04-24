use crate::state::{ApeProcessIds, ApeProcessState};
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
use glenda::cap::Endpoint;

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
        if self.process.session_id == 0 {
            self.process.session_id = ids.pid;
        }
        if self.process.process_group_id == 0 {
            self.process.process_group_id = ids.pid;
        }
        self.process.ids = Some(ids);
    }

    pub fn process_ids(&self) -> Option<ApeProcessIds> {
        self.process.ids
    }

    pub fn set_clear_child_tid(&mut self, ptr: usize) {
        self.process.signal.clear_child_tid = ptr;
    }

    pub fn set_identity(&mut self, uid: usize, euid: usize, gid: usize, egid: usize) {
        self.process.identity.uid = uid as u32;
        self.process.identity.euid = euid as u32;
        self.process.identity.gid = gid as u32;
        self.process.identity.egid = egid as u32;
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

    pub fn ape_endpoint(&self) -> Option<Endpoint> {
        self.process.ape_endpoint
    }

    pub fn set_ape_endpoint(&mut self, ep: Endpoint) {
        self.process.ape_endpoint = Some(ep);
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
