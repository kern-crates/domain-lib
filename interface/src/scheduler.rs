use alloc::vec::Vec;

use downcast_rs::{impl_downcast, DowncastSync};
use gproxy::proxy;
use rref::RRef;
use task_meta::{TaskBasicInfo, TaskContext, TaskSchedulingInfo};

use super::AlienResult;
use crate::Basic;

#[proxy(SchedulerDomainProxy)]
pub trait SchedulerDomain: Basic + DowncastSync {
    fn init(&self) -> AlienResult<()>;
    /// add one task to scheduler
    fn add_task(&self, scheduling_info: RRef<TaskSchedulingInfo>) -> AlienResult<()>;
    /// The next task to run
    fn fetch_task(&self, info: RRef<TaskSchedulingInfo>) -> AlienResult<RRef<TaskSchedulingInfo>>;
    fn dump_meta_data(&self, data: &mut SchedulerDataContainer) -> AlienResult<()>;
}

impl_downcast!(sync SchedulerDomain);

#[derive(Debug, Copy, Clone, Default)]
pub struct CpuLocalData {
    pub cpu_context: TaskContext,
    pub task: Option<TaskData>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct KStackData {
    pub kstack_top: usize,
    pub pages: usize,
}
#[derive(Debug, Copy, Clone)]
pub struct TaskData {
    pub task_meta: TaskBasicInfo,
    pub kstack_data: KStackData,
}

#[derive(Debug, Default)]
pub struct SchedulerDataContainer {
    pub cpu_local: CpuLocalData,
    pub task_wait_queue: Vec<TaskData>,
    pub task_ready_queue: Vec<TaskData>,
}
