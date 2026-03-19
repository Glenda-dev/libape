use glenda::cap::{CapPtr, Endpoint};

// Assigning a specific APE slot for the injected APE Endpoint cap
pub const APE_SLOT: CapPtr = CapPtr::from(11);
pub const APE_CAP: Endpoint = Endpoint::from(APE_SLOT);
