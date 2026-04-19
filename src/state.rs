use alloc::collections::BTreeMap;
use alloc::string::String;

pub const SLOW_QUEUE_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, Default)]
pub struct ApeProcessIds {
    pub pid: usize,
    pub ppid: usize,
    pub tid: usize,
}

impl ApeProcessIds {
    pub const fn valid(&self) -> bool {
        self.pid != 0 && self.tid != 0
    }
}

#[derive(Debug, Clone)]
pub struct FdEntry {
    pub offset: usize,
    pub flags: u32,
    pub endpoint_cptr: usize,
    pub path: Option<String>,
    pub cloexec: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FdState {
    pub table: BTreeMap<i32, FdEntry>,
    pub next_fd: i32,
    pub cwd: String,
    pub root_dir: String,
}

impl FdState {
    pub fn new() -> Self {
        Self { table: BTreeMap::new(), next_fd: 3, cwd: String::new(), root_dir: String::new() }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryMapEntry {
    pub start: usize,
    pub len: usize,
    pub prot: u32,
    pub flags: u32,
    pub backing_fd: Option<i32>,
    pub backing_offset: usize,
    pub lazy: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryState {
    pub brk_start: usize,
    pub brk_current: usize,
    pub heap_limit: usize,
    pub mmap_base: usize,
    pub mmap_next: usize,
    pub mmap_limit: usize,
    pub maps: BTreeMap<usize, MemoryMapEntry>,
}

impl MemoryState {
    pub fn new() -> Self {
        Self {
            brk_start: 0,
            brk_current: 0,
            heap_limit: usize::MAX,
            mmap_base: 0,
            mmap_next: 0,
            mmap_limit: usize::MAX,
            maps: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SignalAction {
    pub handler: usize,
    pub flags: usize,
    pub restorer: usize,
    pub mask: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SignalState {
    pub actions: BTreeMap<usize, SignalAction>,
    pub blocked: u64,
    pub pending: u64,
    pub clear_child_tid: usize,
}

impl SignalState {
    pub fn new() -> Self {
        Self { actions: BTreeMap::new(), blocked: 0, pending: 0, clear_child_tid: 0 }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FutexBucket {
    pub waiters: usize,
}

#[derive(Debug, Clone, Default)]
pub struct FutexState {
    pub buckets: BTreeMap<usize, FutexBucket>,
}

impl FutexState {
    pub fn new() -> Self {
        Self { buckets: BTreeMap::new() }
    }

    pub fn register_waiter(&mut self, uaddr: usize) {
        let entry = self.buckets.entry(uaddr).or_default();
        entry.waiters = entry.waiters.saturating_add(1);
    }

    pub fn wake_waiters(&mut self, uaddr: usize, max: usize) -> usize {
        let Some(entry) = self.buckets.get_mut(&uaddr) else {
            return 0;
        };

        let woke = core::cmp::min(entry.waiters, max);
        entry.waiters -= woke;
        if entry.waiters == 0 {
            self.buckets.remove(&uaddr);
        }
        woke
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RoutePolicy {
    LocalOnly = 0,
    PreferLocal = 1,
    PreferFallback = 2,
}

impl RoutePolicy {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::LocalOnly,
            2 => Self::PreferFallback,
            _ => Self::PreferLocal,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RouteConfig {
    pub policy: RoutePolicy,
    pub slow_path_enabled: bool,
    pub fallback_enabled: bool,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self { policy: RoutePolicy::PreferLocal, slow_path_enabled: true, fallback_enabled: true }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RouteStats {
    pub local_fast_hits: u64,
    pub local_slow_hits: u64,
    pub slow_enqueued: u64,
    pub fallback_hits: u64,
    pub unsupported_hits: u64,
    pub queue_drops: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SlowSyscallRequest {
    pub id: u64,
    pub sys_num: usize,
    pub args: [usize; 6],
}

#[derive(Debug, Clone, Copy)]
pub struct SlowSyscallResult {
    pub id: u64,
    pub ret: isize,
}

#[derive(Debug, Clone)]
pub struct SlowQueue {
    pending: [Option<SlowSyscallRequest>; SLOW_QUEUE_CAPACITY],
    pending_head: usize,
    pending_tail: usize,
    pending_count: usize,
    results: [Option<SlowSyscallResult>; SLOW_QUEUE_CAPACITY],
    result_count: usize,
    next_id: u64,
}

impl SlowQueue {
    pub const fn new() -> Self {
        Self {
            pending: [None; SLOW_QUEUE_CAPACITY],
            pending_head: 0,
            pending_tail: 0,
            pending_count: 0,
            results: [None; SLOW_QUEUE_CAPACITY],
            result_count: 0,
            next_id: 1,
        }
    }

    pub fn enqueue_request(&mut self, sys_num: usize, args: [usize; 6]) -> Option<u64> {
        if self.pending_count >= SLOW_QUEUE_CAPACITY {
            return None;
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.pending[self.pending_tail] = Some(SlowSyscallRequest { id, sys_num, args });
        self.pending_tail = (self.pending_tail + 1) % SLOW_QUEUE_CAPACITY;
        self.pending_count += 1;
        Some(id)
    }

    pub fn dequeue_request(&mut self) -> Option<SlowSyscallRequest> {
        if self.pending_count == 0 {
            return None;
        }

        let req = self.pending[self.pending_head].take();
        self.pending_head = (self.pending_head + 1) % SLOW_QUEUE_CAPACITY;
        self.pending_count -= 1;
        req
    }

    pub fn push_result(&mut self, id: u64, ret: isize) -> bool {
        if self.result_count >= SLOW_QUEUE_CAPACITY {
            return false;
        }

        if let Some(slot) = self.results.iter_mut().find(|x| x.is_none()) {
            *slot = Some(SlowSyscallResult { id, ret });
            self.result_count += 1;
            return true;
        }

        false
    }

    pub fn take_result(&mut self, id: u64) -> Option<isize> {
        for slot in &mut self.results {
            if let Some(v) = slot
                && v.id == id
            {
                let ret = v.ret;
                *slot = None;
                self.result_count = self.result_count.saturating_sub(1);
                return Some(ret);
            }
        }
        None
    }

    pub fn pending_len(&self) -> usize {
        self.pending_count
    }
}

#[derive(Debug, Clone)]
pub struct ApeProcessState {
    ids: Option<ApeProcessIds>,
    pub fd: FdState,
    pub memory: MemoryState,
    pub signal: SignalState,
    pub futex: FutexState,
    pub uid: usize,
    pub euid: usize,
    pub gid: usize,
    pub egid: usize,
    pub route_config: RouteConfig,
    pub route_stats: RouteStats,
    pub slow_queue: SlowQueue,
    pub service_tid: Option<usize>,
}

impl ApeProcessState {
    pub fn new() -> Self {
        Self {
            ids: None,
            fd: FdState::new(),
            memory: MemoryState::new(),
            signal: SignalState::new(),
            futex: FutexState::new(),
            uid: 0,
            euid: 0,
            gid: 0,
            egid: 0,
            route_config: RouteConfig::default(),
            route_stats: RouteStats::default(),
            slow_queue: SlowQueue::new(),
            service_tid: None,
        }
    }

    pub fn set_ids(&mut self, ids: ApeProcessIds) {
        self.ids = ids.valid().then_some(ids);
    }

    pub fn ids(&self) -> Option<ApeProcessIds> {
        self.ids
    }

    pub fn set_identity(&mut self, uid: usize, euid: usize, gid: usize, egid: usize) {
        self.uid = uid;
        self.euid = euid;
        self.gid = gid;
        self.egid = egid;
    }

    pub fn set_route_config(&mut self, policy: RoutePolicy, slow: bool, fallback: bool) {
        self.route_config =
            RouteConfig { policy, slow_path_enabled: slow, fallback_enabled: fallback };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_queue_round_trip() {
        let mut q = SlowQueue::new();
        let id = q.enqueue_request(123, [1, 2, 3, 4, 5, 6]).expect("enqueue should succeed");
        assert_eq!(q.pending_len(), 1);

        let req = q.dequeue_request().expect("request should exist");
        assert_eq!(req.id, id);
        assert_eq!(req.sys_num, 123);
        assert_eq!(req.args, [1, 2, 3, 4, 5, 6]);

        assert!(q.push_result(id, 77));
        assert_eq!(q.take_result(id), Some(77));
        assert_eq!(q.take_result(id), None);
    }

    #[test]
    fn futex_wait_wake_accounting() {
        let mut f = FutexState::new();
        f.register_waiter(0x1000);
        f.register_waiter(0x1000);
        f.register_waiter(0x1000);

        assert_eq!(f.wake_waiters(0x1000, 2), 2);
        assert_eq!(f.wake_waiters(0x1000, 2), 1);
        assert_eq!(f.wake_waiters(0x1000, 1), 0);
    }

    #[test]
    fn route_policy_decode() {
        assert_eq!(RoutePolicy::from_u8(0), RoutePolicy::LocalOnly);
        assert_eq!(RoutePolicy::from_u8(1), RoutePolicy::PreferLocal);
        assert_eq!(RoutePolicy::from_u8(2), RoutePolicy::PreferFallback);
        assert_eq!(RoutePolicy::from_u8(9), RoutePolicy::PreferLocal);
    }
}
