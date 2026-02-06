use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use spin::Mutex;

pub struct FileHandle {
    // Placeholder for file handle details
    pub cap_idx: usize,
    pub offset: usize,
}

pub struct ApeService {
    pub cwd: Mutex<String>,
    pub fds: Mutex<BTreeMap<usize, FileHandle>>,
}

impl ApeService {
    // Note: BTreeMap::new() is const
    // String::new() is const
    // Mutex::new() is likely const (spin::Mutex)
    pub const fn new() -> Self {
        Self { cwd: Mutex::new(String::new()), fds: Mutex::new(BTreeMap::new()) }
    }
}

// Implement Sync because of global static usage
unsafe impl Sync for ApeService {}
