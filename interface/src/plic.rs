use core::ops::Range;

use downcast_rs::{impl_downcast, DowncastSync};
use gproxy::proxy;
use rref::RRefVec;

use super::AlienResult;
use crate::Basic;
#[proxy(PLICDomainProxy, RwLock, PlicInfo)]
pub trait PLICDomain: Basic + DowncastSync {
    fn init(&self, plic_info: &PlicInfo) -> AlienResult<()>;
    fn handle_irq(&self) -> AlienResult<()>;
    fn register_irq(&self, irq: usize, device_domain_name: &RRefVec<u8>) -> AlienResult<()>;
    fn irq_info(&self, buf: RRefVec<u8>) -> AlienResult<RRefVec<u8>>;
}

impl_downcast!(sync PLICDomain);

#[derive(Clone, Debug)]
pub struct PlicInfo {
    pub device_info: Range<usize>,
    pub ty: PlicType,
}

#[derive(Copy, Clone, Debug)]
pub enum PlicType {
    Qemu,
    SiFive,
}
