#![no_std]
#![no_main]

extern crate alloc;
extern crate glenda;

use glenda::cap::{CapPtr, Endpoint, Reply};
#[cfg(target_arch = "riscv64")]
use glenda::ipc::ThreadControlBlock;
use glenda::ipc::{MsgFlags, MsgTag, UTCB};
use glenda::protocol;

// Use slot 30 and 31 for the daemon's IPC operations
const DAEMON_REPLY_SLOT: CapPtr = CapPtr::from(30);
const DAEMON_RECV_SLOT: CapPtr = CapPtr::from(31);

#[inline]
fn init_thread_tcb_from_registers() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        let tp: usize;
        let tid: usize;
        core::arch::asm!("mv {}, tp", out(reg) tp);
        core::arch::asm!("mv {}, a1", out(reg) tid);
        if tp != 0 {
            let tcb = &mut *(tp as *mut ThreadControlBlock);
            tcb.self_ptr = tp;
            tcb.tid = tid;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    ape::__libape_init(); // Initialize the runtime
    init_thread_tcb_from_registers();

    // 尝试引导
    ensure_bootstrap();

    let ape_ep = Endpoint::from(CapPtr::from(11));
    let reply = Reply::from(DAEMON_REPLY_SLOT);

    loop {
        let mut utcb = unsafe { UTCB::new() };
        utcb.set_reply_window(reply.cap());
        utcb.set_recv_window(DAEMON_RECV_SLOT);

        if let Err(_e) = ape_ep.recv(&mut utcb) {
            continue;
        }

        let tag = utcb.get_msg_tag();
        let proto = tag.proto();
        let label = tag.label();

        if proto == protocol::KERNEL_PROTO && label == protocol::kernel::SYSCALL {
            let arg0 = utcb.get_mr(0);
            let arg1 = utcb.get_mr(1);
            let arg2 = utcb.get_mr(2);
            let arg3 = utcb.get_mr(3);
            let arg4 = utcb.get_mr(4);
            let arg5 = utcb.get_mr(5);
            let sys_num = utcb.get_mr(7);

            let ret = ape::syscall::dispatch_syscall(sys_num, [arg0, arg1, arg2, arg3, arg4, arg5]);

            utcb.clear();
            utcb.set_mr(0, ret as usize);
            let _ = reply.reply(&mut utcb);
        } else {
            // Unhandled IPC, just reply error
            utcb.clear();
            utcb.set_mr(0, -(linux_raw_sys::errno::ENOSYS as isize) as usize);
            let _ = reply.reply(&mut utcb);
        }
    }
}

fn ensure_bootstrap() {
    let global_ape_ep = Endpoint::from(CapPtr::from(12));
    let tag = MsgTag::new(protocol::APE_PROTO, protocol::ape::GET_BOOTSTRAP_STATE, MsgFlags::NONE);
    let utcb = unsafe { UTCB::new() };
    utcb.clear();
    utcb.set_msg_tag(tag);

    if let Ok(_) = global_ape_ep.call(utcb) {
        let pid = utcb.get_mr(0);
        let ppid = utcb.get_mr(1);
        let tid = utcb.get_mr(2);
        ape::syscall::set_process_ids(ape::state::ApeProcessIds { pid, ppid, tid });
        ape::syscall::set_ape_endpoint(global_ape_ep.cap().bits());
        ape::syscall::mark_bootstrap_ready();
    }
}
