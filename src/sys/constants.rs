use glenda::arch::mem::PGSIZE;
use glenda::mem::{HEAP_VA, THREAD_AREA_BASE};
use linux_raw_sys::general::{SIGBUS, SIGILL, SIGSEGV, SIGTRAP, STDERR_FILENO};

pub const FIRST_USER_FD: u32 = STDERR_FILENO + 1;

pub const DEFAULT_MAX_STACK_SIZE: usize = 8 * 1024 * 1024;
pub const DEFAULT_MMAP_BASE: usize = HEAP_VA + 16 * PGSIZE;
pub const DEFAULT_MMAP_LIMIT: usize = THREAD_AREA_BASE - 16 * PGSIZE;
pub const DEFAULT_HEAP_LIMIT: usize = DEFAULT_MMAP_BASE;

pub const SIGILL_EXIT_CODE: usize = 128 + SIGILL as usize;
pub const SIGTRAP_EXIT_CODE: usize = 128 + SIGTRAP as usize;
pub const SIGSEGV_EXIT_CODE: usize = 128 + SIGSEGV as usize;
pub const UNKNOWN_FAULT_EXIT_CODE: usize = 128 + SIGBUS as usize;
