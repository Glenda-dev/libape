//! vDSO client-side implementation for libape

pub const VDSO_GLOBAL_VA: usize = 0x7FFFFFFFF000 - 0x1000;
pub const VDSO_PROCESS_VA: usize = 0x7FFFFFFFF000 - 0x2000;

pub fn get_pid() -> u32 {
    let proc_vdso = unsafe { &*(VDSO_PROCESS_VA as *const VdsoProcess) };
    proc_vdso.pid
}

pub fn get_time_ns() -> u64 {
    let global_vdso = unsafe { &*(VDSO_GLOBAL_VA as *const VdsoGlobal) };
    global_vdso.system_time_ns
}

#[repr(C)]
pub struct VdsoGlobal {
    pub system_time_ns: u64,
    pub boot_time_ns: u64,
    pub cycle_freq: u64,
}

#[repr(C)]
pub struct VdsoProcess {
    pub pid: u32,
    pub ppid: u32,
    pub tgid: u32,
    pub uid: u32,
    pub gid: u32,
}


