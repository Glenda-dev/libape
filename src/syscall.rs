use crate::mem;
use crate::path;
use crate::runtime::{with_runtime, with_runtime_read};
use crate::state::{ApeProcessIds, FdEntry, MemoryMap, MemoryType};
use crate::version::{DOMAINNAME, MACHINE, NODENAME, RELEASE, SYSNAME, VERSION};
use glenda::cap::{CSPACE_CAP, CapPtr, Endpoint};
use glenda::client::FsClient;
use glenda::interface::{FileHandleService, FileSystemService};
use glenda::ipc::{MsgFlags, MsgTag, UTCB};
use glenda::protocol::APE_PROTO;
use glenda::protocol::ape;
use glenda::protocol::fs;
use glenda::protocol::fs::OpenFlags;
use linux_raw_sys::ctypes::c_char;
use linux_raw_sys::errno::{EACCES, EBADF, EFAULT, EINVAL, ENOENT, ENOSYS, EPERM, ESRCH};
use linux_raw_sys::general::*;

#[inline]
const fn errno(code: u32) -> isize {
    -(code as isize)
}

const UTS_STR_LEN: usize = 65;
const FD_CAP_BASE_SLOT: usize = 128;

#[repr(C)]
#[derive(Clone, Copy)]
struct LinuxUtsname {
    sysname: [c_char; UTS_STR_LEN],
    nodename: [c_char; UTS_STR_LEN],
    release: [c_char; UTS_STR_LEN],
    version: [c_char; UTS_STR_LEN],
    machine: [c_char; UTS_STR_LEN],
    domainname: [c_char; UTS_STR_LEN],
}

impl LinuxUtsname {
    fn new() -> Self {
        Self {
            sysname: [0; UTS_STR_LEN],
            nodename: [0; UTS_STR_LEN],
            release: [0; UTS_STR_LEN],
            version: [0; UTS_STR_LEN],
            machine: [0; UTS_STR_LEN],
            domainname: [0; UTS_STR_LEN],
        }
    }
}

fn write_uts_field(dst: &mut [c_char; UTS_STR_LEN], value: &str) {
    let bytes = value.as_bytes();
    let n = core::cmp::min(bytes.len(), UTS_STR_LEN.saturating_sub(1));
    for i in 0..n {
        dst[i] = bytes[i] as c_char;
    }
}

pub fn dispatch_syscall(sys_num: usize, args: [usize; 6]) -> isize {
    match sys_num as u32 {
        // --- Process & Threading ---
        __NR_getpid => local_getpid(),
        __NR_gettid => local_gettid(),
        __NR_getppid => local_getppid(),
        __NR_set_tid_address => local_set_tid_address(args[0]),
        __NR_sched_yield => local_sched_yield(),
        __NR_clone => ape_ipc(ape::CLONE_PROCESS, args),
        __NR_execve => ape_ipc(ape::EXECVE, args),
        __NR_wait4 => ape_ipc(ape::WAIT_PROCESS, args),
        __NR_exit | __NR_exit_group => ape_ipc(ape::EXIT_PROCESS, args),
        __NR_kill => ape_ipc(ape::DELIVER_SIGNAL, args),
        __NR_setsid => local_setsid(),
        __NR_setpgid => local_setpgid(args[0], args[1]),
        __NR_getsid => local_getsid(args[0]),
        __NR_getpgid => local_getpgid(args[0]),

        // --- Memory Management ---
        __NR_brk => local_brk(args[0]),
        __NR_mmap => local_mmap(args[0], args[1], args[2], args[3], args[4] as i32, args[5]),
        __NR_munmap => local_munmap(args[0], args[1]),
        __NR_mprotect => local_mprotect(args[0], args[1], args[2] as u32),

        // --- VFS & File I/O ---
        __NR_openat => {
            local_openat(args[0] as i32, args[1] as *const u8, args[2] as u32, args[3] as u32)
        }
        __NR_read => local_read(args[0] as i32, args[1] as *mut u8, args[2]),
        __NR_write => local_write(args[0] as i32, args[1] as *const u8, args[2]),
        __NR_close => local_close(args[0] as i32),
        __NR_lseek => local_lseek(args[0] as i32, args[1] as i64, args[2]),
        __NR_dup => local_dup(args[0] as i32),
        __NR_dup3 => local_dup3(args[0] as i32, args[1] as i32, args[2] as u32),
        __NR_getdents64 => local_getdents64(args[0] as i32, args[1] as *mut u8, args[2]),
        __NR_newfstatat => {
            local_fstatat(args[0] as i32, args[1] as *const u8, args[2] as *mut u8, args[3] as u32)
        }
        __NR_readlinkat => {
            local_readlinkat(args[0] as i32, args[1] as *const u8, args[2] as *mut u8, args[3])
        }
        __NR_fstat => local_fstat(args[0] as i32, args[1] as *mut u8),
        __NR_mkdirat => local_mkdirat(args[0] as i32, args[1] as *const u8, args[2] as u32),
        __NR_unlinkat => local_unlinkat(args[0] as i32, args[1] as *const u8, args[2] as u32),
        __NR_fcntl => local_fcntl(args[0] as i32, args[1], args[2]),
        __NR_getcwd => local_getcwd(args[0] as *mut u8, args[1]),
        __NR_chdir => local_chdir(args[0] as *const u8),
        __NR_fchdir => local_fchdir(args[0] as i32),
        __NR_chroot => local_chroot(args[0] as *const u8),
        __NR_uname => local_uname(args[0] as *mut u8),

        // --- Identity ---
        __NR_getuid => local_getuid(),
        __NR_geteuid => local_geteuid(),
        __NR_getgid => local_getgid(),
        __NR_getegid => local_getegid(),

        // --- Signals ---
        __NR_rt_sigaction => {
            local_rt_sigaction(args[0], args[1] as *const u8, args[2] as *mut u8, args[3])
        }
        __NR_rt_sigprocmask => {
            local_rt_sigprocmask(args[0], args[1] as *const u64, args[2] as *mut u64, args[3])
        }
        __NR_rt_sigpending => local_rt_sigpending(args[0] as *mut u64, args[1]),

        __NR_getrandom => local_getrandom(args[0] as *mut u8, args[1]),

        _ => errno(ENOSYS),
    }
}

fn ape_ipc(label: usize, args: [usize; 6]) -> isize {
    with_runtime(|rt| {
        let Some(ep) = rt.ape_endpoint() else {
            return errno(ENOSYS);
        };
        let tag = MsgTag::new(APE_PROTO, label, MsgFlags::NONE);
        let utcb = unsafe { UTCB::new() };
        utcb.clear();
        for i in 0..6 {
            utcb.set_mr(i, args[i]);
        }
        utcb.set_msg_tag(tag);
        if let Err(e) = ep.call(utcb) {
            return -(e as isize);
        }
        utcb.get_mr(0) as isize
    })
}

// ----- Local Handlers -----

fn local_getpid() -> isize {
    with_runtime_read(|rt| rt.process_ids().map(|ids| ids.pid as isize).unwrap_or(errno(ENOSYS)))
}

fn local_gettid() -> isize {
    with_runtime_read(|rt| rt.process_ids().map(|ids| ids.tid as isize).unwrap_or(errno(ENOSYS)))
}

fn local_getppid() -> isize {
    with_runtime_read(|rt| rt.process_ids().map(|ids| ids.ppid as isize).unwrap_or(errno(ENOSYS)))
}

fn local_getuid() -> isize {
    with_runtime_read(|rt| rt.process_state().identity.uid as isize)
}
fn local_geteuid() -> isize {
    with_runtime_read(|rt| rt.process_state().identity.euid as isize)
}
fn local_getgid() -> isize {
    with_runtime_read(|rt| rt.process_state().identity.gid as isize)
}
fn local_getegid() -> isize {
    with_runtime_read(|rt| rt.process_state().identity.egid as isize)
}

fn local_set_tid_address(ptr: usize) -> isize {
    with_runtime(|rt| {
        rt.set_clear_child_tid(ptr);
        rt.process_ids().map(|ids| ids.tid as isize).unwrap_or(errno(ENOSYS))
    })
}

fn local_sched_yield() -> isize {
    0
}

fn local_setsid() -> isize {
    with_runtime(|rt| {
        let Some(ids) = rt.process_ids() else {
            return errno(ENOSYS);
        };
        if rt.process_state().process_group_id == ids.pid {
            return errno(EPERM);
        }
        let process = rt.process_state_mut();
        process.session_id = ids.pid;
        process.process_group_id = ids.pid;
        ids.pid as isize
    })
}

fn local_setpgid(target_pid: usize, pgid: usize) -> isize {
    with_runtime(|rt| {
        let Some(ids) = rt.process_ids() else {
            return errno(ENOSYS);
        };
        if target_pid != 0 && target_pid != ids.pid {
            return errno(ESRCH);
        }
        let new_pgid = if pgid == 0 { ids.pid } else { pgid };
        rt.process_state_mut().process_group_id = new_pgid;
        0
    })
}

fn local_getsid(target_pid: usize) -> isize {
    with_runtime_read(|rt| {
        let Some(ids) = rt.process_ids() else {
            return errno(ENOSYS);
        };
        if target_pid != 0 && target_pid != ids.pid {
            return errno(ESRCH);
        }
        rt.process_state().session_id as isize
    })
}

fn local_getpgid(target_pid: usize) -> isize {
    with_runtime_read(|rt| {
        let Some(ids) = rt.process_ids() else {
            return errno(ENOSYS);
        };
        if target_pid != 0 && target_pid != ids.pid {
            return errno(ESRCH);
        }
        rt.process_state().process_group_id as isize
    })
}

fn local_brk(addr: usize) -> isize {
    with_runtime(|rt| {
        let mem = &mut rt.process_state_mut().memory;
        if mem.brk_start == 0 && addr != 0 {
            mem.brk_start = addr;
            mem.brk_current = addr;
        }
        if addr == 0 {
            return mem.brk_current as isize;
        }
        if addr < mem.brk_start || addr > mem.heap_limit {
            return mem.brk_current as isize;
        }
        mem.brk_current = addr;
        mem.brk_current as isize
    })
}

fn local_mmap(addr: usize, len: usize, prot: usize, flags: usize, fd: i32, offset: usize) -> isize {
    with_runtime(|rt| {
        let mem = &mut rt.process_state_mut().memory;
        let start = if addr == 0 {
            let s = mem.mmap_next;
            mem.mmap_next += (len + 0xFFF) & !0xFFF;
            s
        } else {
            addr
        };

        mem.maps.insert(
            start,
            MemoryMap {
                start,
                len,
                prot: prot as u32,
                flags: flags as u32,
                mem_type: if fd == -1 { MemoryType::Anonymous } else { MemoryType::FileBacked },
                backing_fd: if fd == -1 { None } else { Some(fd) },
                backing_offset: offset,
            },
        );
        start as isize
    })
}

fn local_munmap(addr: usize, _len: usize) -> isize {
    with_runtime(|rt| {
        rt.process_state_mut().memory.maps.remove(&addr);
        0
    })
}

fn local_mprotect(addr: usize, _len: usize, prot: u32) -> isize {
    with_runtime(|rt| {
        if let Some(map) = rt.process_state_mut().memory.maps.get_mut(&addr) {
            map.prot = prot;
            0
        } else {
            errno(EINVAL)
        }
    })
}

fn local_read(fd: i32, buf_ptr: *mut u8, len: usize) -> isize {
    with_runtime(|rt| {
        let Some(entry) = rt.process_state_mut().fds.table.get_mut(&fd) else {
            return errno(EBADF);
        };
        let mut client = FsClient::new(entry.endpoint);
        let mut buf = alloc::vec![0u8; len.min(8192)];
        match client.read(glenda::ipc::Badge::null(), entry.offset, &mut buf) {
            Ok(bytes) => {
                if let Err(e) = mem::copy_to_ptr(buf_ptr, &buf[..bytes]) {
                    return e;
                }
                entry.offset += bytes;
                bytes as isize
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_write(fd: i32, buf_ptr: *const u8, len: usize) -> isize {
    with_runtime(|rt| {
        let Some(entry) = rt.process_state_mut().fds.table.get_mut(&fd) else {
            return errno(EBADF);
        };
        let mut client = FsClient::new(entry.endpoint);
        let mut buf = alloc::vec![0u8; len.min(8192)];
        for i in 0..buf.len() {
            buf[i] = unsafe { buf_ptr.add(i).read() };
        }
        match client.write(glenda::ipc::Badge::null(), entry.offset, &buf) {
            Ok(bytes) => {
                entry.offset += bytes;
                bytes as isize
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_close(fd: i32) -> isize {
    with_runtime(|rt| {
        if let Some(entry) = rt.process_state_mut().fds.table.remove(&fd) {
            let mut client = FsClient::new(entry.endpoint);
            let _ = client.close(glenda::ipc::Badge::null());
            0
        } else {
            errno(EBADF)
        }
    })
}

fn local_lseek(fd: i32, offset: i64, whence: usize) -> isize {
    with_runtime(|rt| {
        let Some(entry) = rt.process_state_mut().fds.table.get_mut(&fd) else {
            return errno(EBADF);
        };
        let mut client = FsClient::new(entry.endpoint);
        match client.seek(glenda::ipc::Badge::null(), offset, whence) {
            Ok(new_off) => {
                entry.offset = new_off;
                new_off as isize
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_dup(fd: i32) -> isize {
    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let Some(entry) = state.fds.table.get(&fd).cloned() else {
            return errno(EBADF);
        };
        let new_fd = state.fds.next_fd;
        state.fds.next_fd += 1;
        state.fds.table.insert(new_fd, entry);
        new_fd as isize
    })
}

fn local_dup3(oldfd: i32, newfd: i32, _flags: u32) -> isize {
    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let Some(entry) = state.fds.table.get(&oldfd).cloned() else {
            return errno(EBADF);
        };
        state.fds.table.insert(newfd, entry);
        newfd as isize
    })
}

fn local_openat(_dirfd: i32, path_ptr: *const u8, flags: u32, mode: u32) -> isize {
    let raw_path = match mem::read_cstr(path_ptr, 4096) {
        Ok(s) => s,
        Err(e) => return e,
    };
    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let path = if raw_path.starts_with('/') {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if _dirfd == AT_FDCWD {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if let Some(base) = state.fds.table.get(&_dirfd).and_then(|e| e.path.as_ref()) {
            path::resolve_path(
                &raw_path,
                &state.fds.root_dir,
                if base.starts_with('/') { base } else { &state.fds.cwd },
                "/",
            )
        } else {
            return errno(EBADF);
        };
        let ape_ep = state.ape_endpoint.ok_or(errno(ENOSYS)).unwrap();
        let mut client = FsClient::new(ape_ep);
        let new_fd = state.fds.next_fd;
        let recv_slot = CapPtr::from(FD_CAP_BASE_SLOT + (new_fd as usize));
        let _ = CSPACE_CAP.delete(recv_slot);
        match client.open(
            glenda::ipc::Badge::null(),
            &path,
            OpenFlags::from_bits_truncate(flags as usize),
            mode,
            recv_slot,
        ) {
            Ok(_) => {
                state.fds.next_fd += 1;
                state.fds.table.insert(
                    new_fd,
                    FdEntry {
                        endpoint: Endpoint::from(recv_slot),
                        offset: 0,
                        flags,
                        cloexec: (flags & O_CLOEXEC as u32) != 0,
                        path: Some(path),
                    },
                );
                new_fd as isize
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_getdents64(fd: i32, _buf_ptr: *mut u8, count: usize) -> isize {
    with_runtime(|rt| {
        let Some(entry) = rt.process_state_mut().fds.table.get_mut(&fd) else {
            return errno(EBADF);
        };
        let mut client = FsClient::new(entry.endpoint);
        match client.getdents(glenda::ipc::Badge::null(), count) {
            Ok(dents) => {
                let mut out = alloc::vec![];
                for dent in dents.iter() {
                    let name_len =
                        dent.d_name.iter().position(|b| *b == 0).unwrap_or(dent.d_name.len());
                    let base = 8 + 8 + 2 + 1;
                    let reclen = ((base + name_len + 1 + 7) & !7) as u16;
                    out.extend_from_slice(&(dent.d_ino as u64).to_ne_bytes());
                    out.extend_from_slice(&dent.d_off.to_ne_bytes());
                    out.extend_from_slice(&reclen.to_ne_bytes());
                    out.push(dent.d_type);
                    out.extend_from_slice(&dent.d_name[..name_len]);
                    out.push(0);
                    let aligned = (out.len() + 7) & !7;
                    out.resize(aligned, 0);
                    if out.len() >= count {
                        break;
                    }
                }
                let to_copy = core::cmp::min(out.len(), count);
                if let Err(e) = mem::copy_to_ptr(_buf_ptr, &out[..to_copy]) {
                    return e;
                }
                to_copy as isize
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_fstat(fd: i32, _statbuf_ptr: *mut u8) -> isize {
    with_runtime(|rt| {
        let Some(entry) = rt.process_state_mut().fds.table.get(&fd) else {
            return errno(EBADF);
        };
        let client = FsClient::new(entry.endpoint);
        match client.stat(glenda::ipc::Badge::null()) {
            Ok(st) => {
                let mut out = [0u8; core::mem::size_of::<stat>()];
                let p = out.as_mut_ptr() as *mut stat;
                unsafe {
                    (*p).st_dev = st.dev as _;
                    (*p).st_ino = st.ino as _;
                    (*p).st_mode = st.mode as _;
                    (*p).st_nlink = st.nlink as _;
                    (*p).st_uid = st.uid as _;
                    (*p).st_gid = st.gid as _;
                    (*p).st_rdev = st.rdev as _;
                    (*p).st_size = st.size as _;
                    (*p).st_blksize = st.blksize as _;
                    (*p).st_blocks = st.blocks as _;
                    (*p).st_atime = st.atime as _;
                    (*p).st_mtime = st.mtime as _;
                    (*p).st_ctime = st.ctime as _;
                }
                match mem::copy_to_ptr(_statbuf_ptr, &out) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_fstatat(_dirfd: i32, path_ptr: *const u8, _statbuf_ptr: *mut u8, _flags: u32) -> isize {
    let raw_path = match mem::read_cstr(path_ptr, 4096) {
        Ok(s) => s,
        Err(e) => return e,
    };
    with_runtime(|rt| {
        let state = rt.process_state();
        let path = if raw_path.starts_with('/') {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if _dirfd == AT_FDCWD {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if let Some(base) = state.fds.table.get(&_dirfd).and_then(|e| e.path.as_ref()) {
            path::resolve_path(&raw_path, &state.fds.root_dir, base, "/")
        } else {
            return errno(EBADF);
        };
        let mut client = FsClient::new(rt.ape_endpoint().unwrap());
        match client.stat_path(glenda::ipc::Badge::null(), &path) {
            Ok(st) => {
                let mut out = [0u8; core::mem::size_of::<stat>()];
                let p = out.as_mut_ptr() as *mut stat;
                unsafe {
                    (*p).st_dev = st.dev as _;
                    (*p).st_ino = st.ino as _;
                    (*p).st_mode = st.mode as _;
                    (*p).st_nlink = st.nlink as _;
                    (*p).st_uid = st.uid as _;
                    (*p).st_gid = st.gid as _;
                    (*p).st_rdev = st.rdev as _;
                    (*p).st_size = st.size as _;
                    (*p).st_blksize = st.blksize as _;
                    (*p).st_blocks = st.blocks as _;
                    (*p).st_atime = st.atime as _;
                    (*p).st_mtime = st.mtime as _;
                    (*p).st_ctime = st.ctime as _;
                }
                match mem::copy_to_ptr(_statbuf_ptr, &out) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_readlinkat(_dirfd: i32, path_ptr: *const u8, buf_ptr: *mut u8, bufsiz: usize) -> isize {
    if bufsiz == 0 {
        return 0;
    }
    if buf_ptr.is_null() {
        return errno(EFAULT);
    }

    let raw_path = match mem::read_cstr(path_ptr, 4096) {
        Ok(s) => s,
        Err(e) => return e,
    };
    with_runtime(|rt| {
        let state = rt.process_state();
        let path = if raw_path.starts_with('/') {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if _dirfd == AT_FDCWD {
            path::resolve_path(&raw_path, &state.fds.root_dir, &state.fds.cwd, "/")
        } else if let Some(base) = state.fds.table.get(&_dirfd).and_then(|e| e.path.as_ref()) {
            path::resolve_path(&raw_path, &state.fds.root_dir, base, "/")
        } else {
            return errno(EBADF);
        };

        let mut client = FsClient::new(rt.ape_endpoint().unwrap());
        match client.readlink_path(glenda::ipc::Badge::null(), &path) {
            Ok(target) => {
                let bytes = target.as_bytes();
                let n = core::cmp::min(bufsiz, bytes.len());
                match mem::copy_to_ptr(buf_ptr, &bytes[..n]) {
                    Ok(()) => n as isize,
                    Err(e) => e,
                }
            }
            Err(e) => -(e as isize),
        }
    })
}

fn local_mkdirat(_dirfd: i32, path_ptr: *const u8, mode: u32) -> isize {
    let path = match mem::read_cstr(path_ptr, 4096) {
        Ok(s) => s,
        Err(e) => return e,
    };
    with_runtime(|rt| {
        let mut client = FsClient::new(rt.ape_endpoint().unwrap());
        match client.mkdir(glenda::ipc::Badge::null(), &path, mode) {
            Ok(_) => 0,
            Err(e) => -(e as isize),
        }
    })
}

fn local_unlinkat(_dirfd: i32, path_ptr: *const u8, _flags: u32) -> isize {
    let path = match mem::read_cstr(path_ptr, 4096) {
        Ok(s) => s,
        Err(e) => return e,
    };
    with_runtime(|rt| {
        let mut client = FsClient::new(rt.ape_endpoint().unwrap());
        match client.unlink(glenda::ipc::Badge::null(), &path) {
            Ok(_) => 0,
            Err(e) => -(e as isize),
        }
    })
}

fn local_getcwd(buf_ptr: *mut u8, size: usize) -> isize {
    if buf_ptr.is_null() {
        return errno(EFAULT);
    }
    if size == 0 {
        return errno(EINVAL);
    }
    with_runtime_read(|rt| {
        let state = rt.process_state();
        let guest_cwd = path::path_inside_root(&state.fds.cwd, &state.fds.root_dir, "/")
            .unwrap_or_else(|| alloc::string::String::from("/"));
        match mem::write_cstr(buf_ptr, size, &guest_cwd) {
            Ok(n) => n as isize,
            Err(e) => e,
        }
    })
}

fn is_dir_mode(mode: u32) -> bool {
    ((mode as usize) & fs::FileType::S_IFMT.bits()) == fs::FileType::S_IFDIR.bits()
}

fn local_chdir(path_ptr: *const u8) -> isize {
    let raw = match mem::read_cstr(path_ptr, 4096) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if raw.is_empty() {
        return errno(ENOENT);
    }

    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let resolved = path::resolve_path(&raw, &state.fds.root_dir, &state.fds.cwd, "/");

        let Some(ape_ep) = state.ape_endpoint else {
            return errno(ENOSYS);
        };
        let mut client = FsClient::new(ape_ep);
        let stat = match client.stat_path(glenda::ipc::Badge::null(), &resolved) {
            Ok(st) => st,
            Err(e) => return -(e as isize),
        };
        if !is_dir_mode(stat.mode) {
            return errno(EINVAL);
        }
        state.fds.cwd = resolved;
        0
    })
}

fn local_fchdir(fd: i32) -> isize {
    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let Some(entry) = state.fds.table.get(&fd).cloned() else {
            return errno(EBADF);
        };
        let Some(path) = entry.path else {
            return errno(EINVAL);
        };

        let client = FsClient::new(entry.endpoint);
        let stat = match client.stat(glenda::ipc::Badge::null()) {
            Ok(st) => st,
            Err(e) => return -(e as isize),
        };
        if !is_dir_mode(stat.mode) {
            return errno(EINVAL);
        }

        state.fds.cwd = path;
        0
    })
}

fn local_chroot(path_ptr: *const u8) -> isize {
    let raw = match mem::read_cstr(path_ptr, 4096) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if raw.is_empty() {
        return errno(ENOENT);
    }

    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let resolved = path::resolve_path(&raw, &state.fds.root_dir, &state.fds.cwd, "/");

        let Some(ape_ep) = state.ape_endpoint else {
            return errno(ENOSYS);
        };
        let mut client = FsClient::new(ape_ep);
        let stat = match client.stat_path(glenda::ipc::Badge::null(), &resolved) {
            Ok(st) => st,
            Err(e) => return -(e as isize),
        };
        if !is_dir_mode(stat.mode) {
            return errno(EINVAL);
        }
        state.fds.root_dir = resolved.clone();
        state.fds.cwd = resolved;
        0
    })
}

fn local_fcntl(fd: i32, cmd: usize, arg: usize) -> isize {
    with_runtime(|rt| {
        let state = rt.process_state_mut();
        let Some(current) = state.fds.table.get(&fd).cloned() else {
            return errno(EBADF);
        };

        match cmd as u32 {
            F_GETFD => {
                if current.cloexec {
                    FD_CLOEXEC as isize
                } else {
                    0
                }
            }
            F_SETFD => {
                if let Some(entry) = state.fds.table.get_mut(&fd) {
                    entry.cloexec = (arg & (FD_CLOEXEC as usize)) != 0;
                    0
                } else {
                    errno(EBADF)
                }
            }
            F_GETFL => current.flags as isize,
            F_SETFL => {
                if let Some(entry) = state.fds.table.get_mut(&fd) {
                    let mut new_flags = entry.flags;
                    let mutable_mask = (O_APPEND | O_NONBLOCK) as u32;
                    new_flags = (new_flags & !mutable_mask) | ((arg as u32) & mutable_mask);
                    entry.flags = new_flags;
                    0
                } else {
                    errno(EBADF)
                }
            }
            F_DUPFD | F_DUPFD_CLOEXEC => {
                let min_fd = arg as i32;
                if min_fd < 0 {
                    return errno(EINVAL);
                }
                let mut new_fd = min_fd;
                while state.fds.table.contains_key(&new_fd) {
                    new_fd = match new_fd.checked_add(1) {
                        Some(v) => v,
                        None => return errno(EINVAL),
                    };
                }
                let mut dup = current.clone();
                dup.cloexec = (cmd as u32) == F_DUPFD_CLOEXEC;
                state.fds.table.insert(new_fd, dup);
                if state.fds.next_fd <= new_fd {
                    state.fds.next_fd = new_fd.saturating_add(1);
                }
                new_fd as isize
            }
            _ => errno(EINVAL),
        }
    })
}

fn local_uname(buf_ptr: *mut u8) -> isize {
    if buf_ptr.is_null() {
        return errno(EFAULT);
    }

    let mut uts = LinuxUtsname::new();
    write_uts_field(&mut uts.sysname, SYSNAME);
    write_uts_field(&mut uts.nodename, NODENAME);
    write_uts_field(&mut uts.release, RELEASE);
    write_uts_field(&mut uts.version, VERSION);
    write_uts_field(&mut uts.machine, MACHINE);
    write_uts_field(&mut uts.domainname, DOMAINNAME);

    let raw = unsafe {
        core::slice::from_raw_parts(
            (&uts as *const LinuxUtsname).cast::<u8>(),
            core::mem::size_of::<LinuxUtsname>(),
        )
    };
    match mem::copy_to_ptr(buf_ptr, raw) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn local_getrandom(buf_ptr: *mut u8, len: usize) -> isize {
    if let Err(e) = mem::write_zeroed(buf_ptr, len) {
        return e;
    }
    len as isize
}

fn local_rt_sigaction(
    signum: usize,
    act_ptr: *const u8,
    oldact_ptr: *mut u8,
    sigsetsize: usize,
) -> isize {
    if sigsetsize != 8 {
        return errno(EINVAL);
    }
    if signum == 0 || signum > 64 {
        return errno(EINVAL);
    }
    if signum == SIGKILL as usize || signum == SIGSTOP as usize {
        return errno(EINVAL);
    }

    with_runtime(|rt| {
        let sig = &mut rt.process_state_mut().signal;

        if !oldact_ptr.is_null() {
            let old = sig.actions.get(&signum).copied().unwrap_or_default();
            let mut raw = [0u8; 32];
            raw[0..8].copy_from_slice(&old.handler.to_ne_bytes());
            raw[8..16].copy_from_slice(&old.flags.to_ne_bytes());
            raw[16..24].copy_from_slice(&old.restorer.to_ne_bytes());
            raw[24..32].copy_from_slice(&old.mask.to_ne_bytes());
            if let Err(e) = mem::copy_to_ptr(oldact_ptr, &raw) {
                return e;
            }
        }

        if !act_ptr.is_null() {
            let mut raw = [0u8; 32];
            for (i, slot) in raw.iter_mut().enumerate() {
                *slot = unsafe { act_ptr.add(i).read() };
            }
            let mut head = [0u8; 8];
            head.copy_from_slice(&raw[0..8]);
            let handler = usize::from_ne_bytes(head);
            head.copy_from_slice(&raw[8..16]);
            let flags = usize::from_ne_bytes(head);
            head.copy_from_slice(&raw[16..24]);
            let restorer = usize::from_ne_bytes(head);
            let mut mask_raw = [0u8; 8];
            mask_raw.copy_from_slice(&raw[24..32]);
            let mask = u64::from_ne_bytes(mask_raw);

            sig.actions
                .insert(signum, crate::state::SignalAction { handler, flags, restorer, mask });
        }

        0
    })
}

fn local_rt_sigprocmask(
    how: usize,
    set_ptr: *const u64,
    oldset_ptr: *mut u64,
    sigsetsize: usize,
) -> isize {
    if sigsetsize != 8 {
        return errno(EINVAL);
    }
    with_runtime(|rt| {
        let sig = &mut rt.process_state_mut().signal;
        if !oldset_ptr.is_null() {
            let _ = mem::write_u64(oldset_ptr, sig.blocked);
        }
        if !set_ptr.is_null() {
            let set = match mem::read_u64(set_ptr) {
                Ok(v) => v,
                Err(e) => return e,
            };
            match how as u32 {
                SIG_BLOCK => sig.blocked |= set,
                SIG_UNBLOCK => sig.blocked &= !set,
                SIG_SETMASK => sig.blocked = set,
                _ => return errno(EINVAL),
            }
        }
        0
    })
}

fn local_rt_sigpending(set_ptr: *mut u64, sigsetsize: usize) -> isize {
    if sigsetsize != 8 {
        return errno(EINVAL);
    }
    with_runtime(|rt| {
        let sig = &rt.process_state().signal;
        let _ = mem::write_u64(set_ptr, sig.pending);
        0
    })
}

pub fn set_process_ids(ids: ApeProcessIds) {
    with_runtime(|rt| rt.set_process_ids(ids));
}
pub fn mark_bootstrap_ready() {
    with_runtime(|rt| rt.set_bootstrap_ready(true));
}
pub fn set_identity(uid: usize, euid: usize, gid: usize, egid: usize) {
    with_runtime(|rt| rt.set_identity(uid, euid, gid, egid));
}
pub fn seed_memory(
    brk_start: usize,
    brk_current: usize,
    heap_limit: usize,
    mmap_base: usize,
    mmap_next: usize,
    mmap_limit: usize,
) {
    with_runtime(|rt| {
        rt.set_memory_seed(brk_start, brk_current, heap_limit, mmap_base, mmap_next, mmap_limit)
    });
}
pub fn set_ape_endpoint(ep_ptr: usize) {
    with_runtime(|rt| rt.set_ape_endpoint(Endpoint::from(CapPtr::from(ep_ptr))));
}
pub fn set_route_policy(_policy: u8, _slow_enabled: bool, _fallback_enabled: bool) {}
pub fn register_service_thread(_tid: usize) {}
pub fn service_poll_once() -> usize {
    0
}
pub fn get_stat(_field: usize) -> u64 {
    0
}
