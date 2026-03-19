use glenda::crt0::init_heap;

#[unsafe(no_mangle)]
pub extern "C" fn __libape_init() {
    init_heap();
    unsafe {
        glenda::arch::runtime::panic_break();
    }
}
