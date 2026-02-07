use crate::ApeService;
use glenda::interface::ProcessService;
use glenda::interface::linux::LinuxProcessService;
use glenda::ipc::Badge;
use glenda::protocol::linux::*;
use glenda::sys;

impl LinuxProcessService for ApeService {
    fn sys_exit(&self, error_code: usize) -> ! {
        sys::exit(error_code);
    }

    fn sys_exit_group(&self, error_code: usize) -> ! {
        sys::exit(error_code);
    }

    fn sys_kill(&self, _pid: usize, _sig: usize) -> isize {
        -ENOSYS
    }

    fn sys_getpid(&self) -> isize {
        let mut proc = self.proc.lock();
        proc.get_pid(Badge::null()).unwrap_or(0) as isize
    }

    fn sys_getppid(&self) -> isize {
        0
    }

    fn sys_getuid(&self) -> isize {
        0
    }

    fn sys_geteuid(&self) -> isize {
        0
    }

    fn sys_getgid(&self) -> isize {
        0
    }

    fn sys_getegid(&self) -> isize {
        0
    }

    fn sys_gettid(&self) -> isize {
        self.sys_getpid()
    }

    fn sys_clone(
        &self,
        _flags: usize,
        _stack: usize,
        _ptid: *mut u32,
        _tls: usize,
        _ctid: *mut u32,
    ) -> isize {
        -ENOSYS
    }
    fn sys_execve(
        &self,
        _pathname: *const u8,
        _argv: *const *const u8,
        _envp: *const *const u8,
    ) -> isize {
        -ENOSYS
    }
    fn sys_wait4(
        &self,
        _pid: isize,
        _wstatus: *mut i32,
        _options: usize,
        _rusage: *mut u8,
    ) -> isize {
        -ENOSYS
    }
    fn sys_prlimit64(
        &self,
        _pid: usize,
        _resource: usize,
        _new_limit: *const u8,
        _old_limit: *mut u8,
    ) -> isize {
        -ENOSYS
    }
}
