use alloc::string::String;
use linux_raw_sys::errno::{EFAULT, EINVAL, ENAMETOOLONG};

#[inline]
const fn errno(code: u32) -> isize {
    -(code as isize)
}

pub fn copy_to_ptr(dst: *mut u8, src: &[u8]) -> Result<(), isize> {
    if src.is_empty() {
        return Ok(());
    }

    if dst.is_null() {
        return Err(errno(EFAULT));
    }

    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
    }
    Ok(())
}

pub fn write_zeroed(dst: *mut u8, len: usize) -> Result<(), isize> {
    if len == 0 {
        return Ok(());
    }

    if dst.is_null() {
        return Err(errno(EFAULT));
    }

    unsafe {
        core::ptr::write_bytes(dst, 0, len);
    }
    Ok(())
}

pub fn read_u64(ptr: *const u64) -> Result<u64, isize> {
    if ptr.is_null() {
        return Err(errno(EFAULT));
    }

    Ok(unsafe { ptr.read_unaligned() })
}

pub fn write_u64(ptr: *mut u64, value: u64) -> Result<(), isize> {
    if ptr.is_null() {
        return Err(errno(EFAULT));
    }

    unsafe {
        ptr.write_unaligned(value);
    }
    Ok(())
}

pub fn read_cstr(ptr: *const u8, max_len: usize) -> Result<String, isize> {
    if ptr.is_null() {
        return Err(errno(EFAULT));
    }
    if max_len == 0 {
        return Err(errno(EINVAL));
    }

    let mut out = alloc::vec::Vec::new();
    for i in 0..max_len {
        let b = unsafe { ptr.add(i).read() };
        if b == 0 {
            return String::from_utf8(out).map_err(|_| errno(EINVAL));
        }
        out.push(b);
    }

    Err(errno(ENAMETOOLONG))
}

pub fn write_cstr(dst: *mut u8, dst_len: usize, s: &str) -> Result<usize, isize> {
    let need = s.len().saturating_add(1);
    if dst_len < need {
        return Err(errno(linux_raw_sys::errno::ERANGE));
    }

    copy_to_ptr(dst, s.as_bytes())?;
    unsafe {
        dst.add(s.len()).write(0);
    }
    Ok(need)
}
