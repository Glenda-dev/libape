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
        utcb.clear();
        // 按 nabcdef( + 0 填充第 8 个 MR )的顺序写入，和 kernel fault 转发格式保持一致。
        set_mrs!(&mut utcb, sys_num, args[0], args[1], args[2], args[3], args[4], args[5], 0usize);
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
