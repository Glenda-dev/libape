use alloc::collections::BTreeMap;
use alloc::string::String;
use glenda::client::{FsClient, ProcessClient, ResourceClient};
use glenda::sync::mutex::Mutex;
use glenda::sys::MONITOR_CAP;

pub struct FileHandle {
    pub cap_idx: usize,
    pub offset: u64,
}

pub struct ApeService {
    pub cwd: Mutex<String>,
    pub fds: Mutex<BTreeMap<usize, FileHandle>>,
    pub fs: Mutex<FsClient>,
    pub proc: Mutex<ProcessClient>,
    pub res: Mutex<ResourceClient>,
}

impl ApeService {
    // Note: BTreeMap::new() is const
    // String::new() is const
    // Mutex::new() is likely const (spin::Mutex)
    pub const fn new() -> Self {
        Self {
            cwd: Mutex::new(String::new()),
            fds: Mutex::new(BTreeMap::new()),
            fs: Mutex::new(FsClient::new(MONITOR_CAP)),
            proc: Mutex::new(ProcessClient::new(MONITOR_CAP)),
            res: Mutex::new(ResourceClient::new(MONITOR_CAP)),
        }
    }
}

// Implement Sync because of global static usage
unsafe impl Sync for ApeService {}
