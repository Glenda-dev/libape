use alloc::collections::BTreeMap;
use alloc::string::String;
use glenda::cap::{CapPtr, Endpoint};
use glenda::protocol::auth::IdentityInfo;

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

#[derive(Debug, Clone, Copy, Default)]
pub struct SignalAction {
    pub handler: usize,
    pub flags: usize,
    pub restorer: usize,
    pub mask: u64,
}

#[derive(Debug, Clone)]
pub struct FdEntry {
    pub endpoint: Endpoint,
    pub offset: usize,
    pub flags: u32,
    pub cloexec: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FdTable {
    pub table: BTreeMap<i32, FdEntry>,
    pub next_fd: i32,
    pub cwd: String,
    pub root_dir: String,
}

impl FdTable {
    pub fn new() -> Self {
        Self {
            table: BTreeMap::new(),
            next_fd: 0,
            cwd: String::from("/"),
            root_dir: String::from("/"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Image,
    Stack,
    Heap,
    Anonymous,
    FileBacked,
}

#[derive(Debug, Clone)]
pub struct MemoryMap {
    pub start: usize,
    pub len: usize,
    pub prot: u32,
    pub flags: u32,
    pub mem_type: MemoryType,
    pub backing_fd: Option<i32>,
    pub backing_offset: usize,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryState {
    pub brk_start: usize,
    pub brk_current: usize,
    pub heap_limit: usize,
    pub mmap_base: usize,
    pub mmap_next: usize,
    pub mmap_limit: usize,
    pub maps: BTreeMap<usize, MemoryMap>,
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

#[derive(Debug, Clone, Default)]
pub struct SignalState {
    pub actions: BTreeMap<usize, SignalAction>,
    pub blocked: u64,
    pub pending: u64,
    pub clear_child_tid: usize,
}

impl SignalState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct FutexState {
    pub waiters: BTreeMap<usize, usize>, // uaddr -> waiter_count
}

impl FutexState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone)]
pub struct ApeProcessState {
    pub ids: Option<ApeProcessIds>,
    pub session_id: usize,
    pub process_group_id: usize,
    pub identity: IdentityInfo,
    pub fds: FdTable,
    pub memory: MemoryState,
    pub signal: SignalState,
    pub futex: FutexState,
    pub ape_endpoint: Option<Endpoint>,
}

impl ApeProcessState {
    pub fn new() -> Self {
        Self {
            ids: None,
            session_id: 0,
            process_group_id: 0,
            identity: IdentityInfo::default(),
            fds: FdTable::new(),
            memory: MemoryState::new(),
            signal: SignalState::new(),
            futex: FutexState::new(),
            ape_endpoint: None,
        }
    }
}
