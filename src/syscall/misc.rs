use crate::ApeService;
use glenda::interface::linux::{LinuxMiscService, LinuxTimeService};
use glenda::protocol::linux::*;

impl LinuxTimeService for ApeService {
    fn sys_clock_gettime(&self, _clockid: usize, _tp: *mut u8) -> isize {
        -ENOSYS
    }
}

use crate::metadata;

impl LinuxMiscService for ApeService {
    fn sys_uname(&self, buf: *mut u8) -> isize {
        if buf.is_null() {
            return -EFAULT;
        }

        // struct utsname {
        //     char sysname[65];
        //     char nodename[65];
        //     char release[65];
        //     char version[65];
        //     char machine[65];
        //     char domainname[65];
        // };
        // Total size: 65 * 6 = 390 bytes

        let fill_field = |offset: usize, s: &str| unsafe {
            let dest = buf.add(offset * 65);
            // Zero out field first
            core::ptr::write_bytes(dest, 0, 65);
            // Copy string bytes
            let len = core::cmp::min(s.len(), 64);
            core::ptr::copy_nonoverlapping(s.as_ptr(), dest, len);
        };

        fill_field(0, metadata::SYSNAME); // sysname
        fill_field(1, metadata::NODENAME); // nodename
        fill_field(2, metadata::RELEASE); // release
        fill_field(3, metadata::VERSION); // version
        fill_field(4, metadata::MACHINE); // machine
        fill_field(5, metadata::DOMAINNAME); // domainname

        0
    }
}
