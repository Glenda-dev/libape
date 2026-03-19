use crate::cap::APE_CAP;
use glenda::cap::Endpoint;
use glenda::error::Error;
use glenda::ipc::{MsgFlags, MsgTag, UTCB};

pub struct ApeClient {
    ep: Endpoint,
}

impl ApeClient {
    pub const fn new() -> Self {
        Self { ep: APE_CAP }
    }

    pub fn invoke_syscall(&self, sys_num: usize, args: [usize; 6]) -> Result<isize, Error> {
        let mut utcb = unsafe { UTCB::new() };
        utcb.set_mr(0, sys_num);
        for i in 0..6 {
            utcb.set_mr(i + 1, args[i]);
        }
        let tag = MsgTag::new(
            glenda::protocol::KERNEL_PROTO,
            glenda::protocol::kernel::SYSCALL,
            MsgFlags::empty(),
        );
        utcb.set_msg_tag(tag);
        self.ep.call(&mut utcb)?;
        Ok(utcb.get_mr(0) as isize)
    }
}
