use linux_raw_sys::errno::ENOSYS;

#[inline]
const fn errno(code: u32) -> isize {
    -(code as isize)
}

#[cfg(target_arch = "riscv64")]
pub fn invoke_linux_syscall(sys_num: usize, args: [usize; 6]) -> isize {
    use core::arch::asm;

    let ret: usize;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") args[0] => ret,
            in("a1") args[1],
            in("a2") args[2],
            in("a3") args[3],
            in("a4") args[4],
            in("a5") args[5],
            in("a7") sys_num,
            options(nostack),
        );
    }
    ret as isize
}

#[cfg(not(target_arch = "riscv64"))]
pub fn invoke_linux_syscall(_sys_num: usize, _args: [usize; 6]) -> isize {
    errno(ENOSYS)
}
