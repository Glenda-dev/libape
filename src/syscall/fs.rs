use crate::ApeService;
use crate::ape::FileHandle;
use glenda::cap::{CapPtr, Endpoint};
use glenda::interface::linux::LinuxFileSystemService;
use glenda::interface::{FileHandleService, FileSystemService, PipeService};
use glenda::protocol::fs::OpenFlags;
use glenda::protocol::linux::*;

impl LinuxFileSystemService for ApeService {
    fn sys_getcwd(&self, buf: *mut u8, size: usize) -> isize {
        let cwd = self.cwd.lock();
        let bytes = cwd.as_bytes();
        if bytes.len() + 1 > size {
            return -ERANGE;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
            *buf.add(bytes.len()) = 0;
        }
        bytes.len() as isize
    }
    fn sys_dup(&self, _oldfd: usize) -> isize {
        -ENOSYS
    }
    fn sys_dup3(&self, _oldfd: usize, _newfd: usize, _flags: usize) -> isize {
        -ENOSYS
    }
    fn sys_mkdirat(&self, _dirfd: usize, path: *const u8, mode: usize) -> isize {
        let path_str = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path, len))
        };
        let mut fs = self.fs.lock();
        match fs.mkdir(path_str, mode as u32) {
            Ok(_) => 0,
            Err(_) => -EACCES,
        }
    }
    fn sys_unlinkat(&self, _dirfd: usize, path: *const u8, _flags: usize) -> isize {
        let path_str = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path, len))
        };
        let mut fs = self.fs.lock();
        match fs.unlink(path_str) {
            Ok(_) => 0,
            Err(_) => -ENOENT,
        }
    }
    fn sys_chdir(&self, path: *const u8) -> isize {
        let path_str = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path, len))
        };
        // In a real system we should check if it exists
        let mut cwd = self.cwd.lock();
        *cwd = path_str.into();
        0
    }
    fn sys_openat(&self, _dirfd: usize, path: *const u8, flags: usize, mode: usize) -> isize {
        let path_str = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path, len))
        };

        let mut fs = self.fs.lock();
        let glenda_flags = OpenFlags::from_bits_truncate(flags);
        match fs.open(path_str, glenda_flags, mode as u32) {
            Ok(cap_idx) => {
                let mut fds = self.fds.lock();
                let fd = fds.keys().last().map(|k| k + 1).unwrap_or(3);
                fds.insert(fd, FileHandle { cap_idx, offset: 0 });
                fd as isize
            }
            Err(_) => -ENOENT,
        }
    }
    fn sys_close(&self, fd: usize) -> isize {
        let mut fds = self.fds.lock();
        if let Some(handle) = fds.remove(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            let _ = file_client.close();
            0
        } else {
            -EBADF
        }
    }
    fn sys_pipe2(&self, pipefd: *mut i32, _flags: usize) -> isize {
        let mut fs = self.fs.lock();
        match fs.pipe() {
            Ok((read_cap, write_cap)) => {
                let mut fds = self.fds.lock();
                let fd1 = fds.keys().last().map(|k| k + 1).unwrap_or(3);
                fds.insert(fd1, FileHandle { cap_idx: read_cap, offset: 0 });
                let fd2 = fd1 + 1;
                fds.insert(fd2, FileHandle { cap_idx: write_cap, offset: 0 });
                unsafe {
                    *pipefd = fd1 as i32;
                    *pipefd.add(1) = fd2 as i32;
                }
                0
            }
            Err(_) => -EMFILE,
        }
    }
    fn sys_getdents64(&self, fd: usize, dirp: *mut u8, count: usize) -> isize {
        let mut fds = self.fds.lock();
        if let Some(handle) = fds.get_mut(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            match file_client.getdents(count / 264) {
                // Approximate number of entries based on max size
                Ok(entries) => {
                    let mut offset = 0;
                    for entry in entries {
                        let name_len = entry.d_name.iter().position(|&b| b == 0).unwrap_or(256);
                        let reclen = (8 + 8 + 2 + 1 + name_len + 1 + 7) & !7;
                        if offset + reclen > count {
                            break;
                        }
                        unsafe {
                            let curr = dirp.add(offset);
                            *(curr as *mut u64) = entry.d_ino;
                            *(curr.add(8) as *mut i64) = (offset + reclen) as i64;
                            *(curr.add(16) as *mut u16) = reclen as u16;
                            *(curr.add(18) as *mut u8) = entry.d_type;
                            core::ptr::copy_nonoverlapping(
                                entry.d_name.as_ptr(),
                                curr.add(19),
                                name_len,
                            );
                            *curr.add(19 + name_len) = 0;
                        }
                        offset += reclen;
                    }
                    offset as isize
                }
                Err(_) => -EIO,
            }
        } else {
            -EBADF
        }
    }
    fn sys_lseek(&self, fd: usize, offset: isize, whence: usize) -> isize {
        let mut fds = self.fds.lock();
        if let Some(handle) = fds.get_mut(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            match file_client.seek(offset as i64, whence) {
                Ok(new_offset) => {
                    handle.offset = new_offset;
                    new_offset as isize
                }
                Err(_) => -EINVAL,
            }
        } else {
            -EBADF
        }
    }

    fn sys_read(&self, fd: usize, buf: *mut u8, count: usize) -> isize {
        let mut fds = self.fds.lock();
        if let Some(handle) = fds.get_mut(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            let slice = unsafe { core::slice::from_raw_parts_mut(buf, count) };
            match file_client.read(handle.offset, slice) {
                Ok(n) => {
                    handle.offset += n as u64;
                    n as isize
                }
                Err(_) => -EIO,
            }
        } else {
            -EBADF
        }
    }

    fn sys_write(&self, fd: usize, buf: *const u8, count: usize) -> isize {
        if fd == 1 || fd == 2 {
            let slice = unsafe { core::slice::from_raw_parts(buf, count) };
            if let Ok(s) = core::str::from_utf8(slice) {
                glenda::print!("{}", s);
                return count as isize;
            }
        }

        let mut fds = self.fds.lock();
        if let Some(handle) = fds.get_mut(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            let slice = unsafe { core::slice::from_raw_parts(buf, count) };
            match file_client.write(handle.offset, slice) {
                Ok(n) => {
                    handle.offset += n as u64;
                    n as isize
                }
                Err(_) => -EIO,
            }
        } else {
            -EBADF
        }
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
        path: *const u8,
        statbuf: *mut u8,
        _flags: usize,
    ) -> isize {
        let path_str = unsafe {
            let mut len = 0;
            while *path.add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path, len))
        };
        let mut fs = self.fs.lock();
        match fs.stat_path(path_str) {
            Ok(stat) => {
                self.fill_linux_stat(&stat, statbuf);
                0
            }
            Err(_) => -ENOENT,
        }
    }
    fn sys_fstat(&self, fd: usize, statbuf: *mut u8) -> isize {
        let mut fds = self.fds.lock();
        if let Some(handle) = fds.get_mut(&fd) {
            let cap = CapPtr::from(handle.cap_idx);
            let mut file_client = FsClient::new(Endpoint::from(cap));
            match file_client.stat() {
                Ok(stat) => {
                    self.fill_linux_stat(&stat, statbuf);
                    0
                }
                Err(_) => -EIO,
            }
        } else {
            -EBADF
        }
    }
}

impl ApeService {
    fn fill_linux_stat(&self, stat: &glenda::protocol::fs::Stat, buf: *mut u8) {
        unsafe {
            let s = buf as *mut u64;
            *s.add(0) = stat.dev;
            *s.add(1) = stat.ino;
            *(buf.add(16) as *mut u32) = stat.mode;
            *(buf.add(20) as *mut u32) = stat.nlink;
            *(buf.add(24) as *mut u32) = stat.uid;
            *(buf.add(28) as *mut u32) = stat.gid;
            *s.add(4) = 0; // st_rdev
            *s.add(5) = 0; // __pad
            *s.add(6) = stat.size;
            *(buf.add(56) as *mut i32) = stat.blksize as i32;
            *s.add(8) = stat.blocks as u64;

            let ts = buf.add(72) as *mut i64;
            *ts.add(0) = stat.atime_sec;
            *ts.add(1) = stat.atime_nsec;
            *ts.add(2) = stat.mtime_sec;
            *ts.add(3) = stat.mtime_nsec;
            *ts.add(4) = stat.ctime_sec;
            *ts.add(5) = stat.ctime_nsec;
        }
    }
}
