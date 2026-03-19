use crate::client::ApeClient;
use crate::sys::constants::*;

#[unsafe(no_mangle)]
pub extern "C" fn __libape_syscall_dispatch(
    n: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
) -> isize {
    let client = ApeClient::new();
    match client.invoke_syscall(n, [a, b, c, d, e, f]) {
        Ok(ret) => ret,
        Err(_) => -ENOSYS,
    }
}
