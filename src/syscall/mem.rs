use crate::ApeService;
use glenda::interface::linux::LinuxMemoryService;
use glenda::protocol::linux::*;
use glenda::sys;

impl LinuxMemoryService for ApeService {
    fn sys_brk(&self, brk: usize) -> isize {
        if brk == 0 {
            match sys::sbrk(0) {
                Ok(addr) => addr as isize,
                Err(_) => -ENOMEM,
            }
        } else {
            match sys::sbrk(0) {
                Ok(current) => {
                    if brk > current {
                        match sys::sbrk(brk - current) {
                            Ok(new_addr) => new_addr as isize,
                            Err(_) => -ENOMEM,
                        }
                    } else {
                        current as isize
                    }
                }
                Err(_) => -ENOMEM,
            }
        }
    }

    fn sys_munmap(&self, _addr: usize, _length: usize) -> isize {
        0
    }
    fn sys_mmap(
        &self,
        _addr: usize,
        _length: usize,
        _prot: usize,
        _flags: usize,
        _fd: usize,
        _offset: usize,
    ) -> isize {
        -ENOSYS
    }
    fn sys_mprotect(&self, _addr: usize, _length: usize, _prot: usize) -> isize {
        0
    }
}
