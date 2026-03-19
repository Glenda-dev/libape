use glenda::arch::ARCH;

pub const SYSNAME: &str = "Linux-glenda";
pub const NODENAME: &str = "glenda";
pub const RELEASE: &str = "5.19.0-glenda";
pub const VERSION: &str = env!("BUILD_TIMESTAMP");
pub const MACHINE: &str = ARCH;
pub const DOMAINNAME: &str = "(none)";
