use alloc::vec::Vec;

use downcast_rs::{impl_downcast, DowncastSync};
use gproxy::proxy;
use rref::RRef;
use task_meta::TaskSchedulingInfo;

use super::AlienResult;
use crate::Basic;

#[proxy(SchedulerDomainProxy)]
pub trait SchedulerDomain: Basic + DowncastSync {
    fn init(&self) -> AlienResult<()>;
    /// add one task to scheduler
    fn add_task(&self, scheduling_info: RRef<TaskSchedulingInfo>) -> AlienResult<()>;
    /// The next task to run
    fn fetch_task(&self, info: RRef<TaskSchedulingInfo>) -> AlienResult<RRef<TaskSchedulingInfo>>;
    fn dump_meta_data(&self) -> AlienResult<Vec<RRef<TaskSchedulingInfo>>>;
    fn rebuild_from_meta_data(
        &self,
        meta_data: &mut Vec<RRef<TaskSchedulingInfo>>,
    ) -> AlienResult<()>;
}

impl_downcast!(sync SchedulerDomain);
