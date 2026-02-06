use crate::ApeService;
use glenda::interface::linux::LinuxFileSystemService;
use glenda::protocol::linux::*;

impl LinuxFileSystemService for ApeService {
    fn sys_getcwd(&self, _buf: *mut u8, _size: usize) -> isize {
        -ENOSYS
    }
    fn sys_dup(&self, _oldfd: usize) -> isize {
        -ENOSYS
    }
    fn sys_dup3(&self, _oldfd: usize, _newfd: usize, _flags: usize) -> isize {
        -ENOSYS
    }
    fn sys_mkdirat(&self, _dirfd: usize, _path: *const u8, _mode: usize) -> isize {
        -ENOSYS
    }
    fn sys_unlinkat(&self, _dirfd: usize, _path: *const u8, _flags: usize) -> isize {
        -ENOSYS
    }
    fn sys_chdir(&self, _path: *const u8) -> isize {
        -ENOSYS
    }
    fn sys_openat(&self, _dirfd: usize, _path: *const u8, _flags: usize, _mode: usize) -> isize {
        -ENOSYS
    }
    fn sys_close(&self, _fd: usize) -> isize {
        -ENOSYS
    }
    fn sys_pipe2(&self, _pipefd: *mut i32, _flags: usize) -> isize {
        -ENOSYS
    }
    fn sys_getdents64(&self, _fd: usize, _dirp: *mut u8, _count: usize) -> isize {
        -ENOSYS
    }
    fn sys_lseek(&self, _fd: usize, _offset: isize, _whence: usize) -> isize {
        -ENOSYS
    }

    fn sys_read(&self, _fd: usize, _buf: *mut u8, _count: usize) -> isize {
        -ENOSYS
    }

    fn sys_write(&self, fd: usize, buf: *const u8, count: usize) -> isize {
        if fd == 1 || fd == 2 {
            let slice = unsafe { core::slice::from_raw_parts(buf, count) };
            if let Ok(s) = core::str::from_utf8(slice) {
                glenda::print!("{}", s);
                return count as isize;
            }
        }
        -ENOSYS
    }

    fn sys_readlinkat(
        &self,
        _dirfd: usize,
        _path: *const u8,
        _buf: *mut u8,
        _bufsize: usize,
    ) -> isize {
        -ENOSYS
    }
    fn sys_newfstatat(
        &self,
        _dirfd: usize,
        _path: *const u8,
        _statbuf: *mut u8,
        _flags: usize,
    ) -> isize {
        -ENOSYS
    }
    fn sys_fstat(&self, _fd: usize, _statbuf: *mut u8) -> isize {
        -ENOSYS
    }
}
