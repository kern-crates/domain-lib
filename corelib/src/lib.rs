#![no_std]
extern crate alloc;

#[cfg(feature = "core_impl")]
pub use core_impl::*;
use interface::{DomainType, DomainTypeRaw};
use pconst::LinuxErrno;
use spin::Once;
use task_meta::{OperationResult, TaskOperation};

pub type AlienError = LinuxErrno;
pub type AlienResult<T> = Result<T, LinuxErrno>;

pub mod constants {
    pub use pconst::*;
    pub const AT_FDCWD: isize = -100isize;
    #[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq, Hash, Ord)]
    pub struct DeviceId {
        major: u32,
        minor: u32,
    }

    impl DeviceId {
        pub fn new(major: u32, minor: u32) -> Self {
            Self { major, minor }
        }
        pub fn id(&self) -> u64 {
            ((self.major as u64) << 32) | (self.minor as u64)
        }
    }

    impl From<u64> for DeviceId {
        fn from(id: u64) -> Self {
            Self {
                major: (id >> 32) as u32,
                minor: (id & 0xffffffff) as u32,
            }
        }
    }
}

pub trait CoreFunction: Send + Sync {
    fn sys_alloc_pages(&self, domain_id: u64, n: usize) -> *mut u8;
    fn sys_free_pages(&self, domain_id: u64, p: *mut u8, n: usize);
    fn sys_write_console(&self, s: &str);
    fn sys_backtrace(&self, domain_id: u64);
    fn sys_trampoline_addr(&self) -> usize;
    fn sys_kernel_satp(&self) -> usize;
    fn sys_trap_from_user(&self) -> usize;
    fn sys_trap_to_user(&self) -> usize;
    /// This func will be deleted
    fn blk_crash_trick(&self) -> bool;
    fn sys_get_domain(&self, name: &str) -> Option<DomainType>;
    fn sys_create_domain(&self, identifier: &str) -> Option<DomainType>;
    /// Register a new domain with the given name and type
    fn sys_register_domain(&self, ident: &str, ty: DomainTypeRaw, data: &[u8]) -> AlienResult<()>;
    /// Replace the old domain with the new domain
    fn sys_update_domain(
        &self,
        old_domain_name: &str,
        new_domain_name: &str,
        ty: DomainTypeRaw,
    ) -> AlienResult<()>;
    fn sys_reload_domain(&self, domain_name: &str) -> AlienResult<()>;
    fn vaddr_to_paddr_in_kernel(&self, vaddr: usize) -> AlienResult<usize>;
    fn task_op(&self, op: TaskOperation) -> AlienResult<OperationResult>;
}

#[cfg(feature = "core_impl")]
mod core_impl {
    use alloc::boxed::Box;

    use interface::{DomainType, DomainTypeRaw};
    use spin::Once;
    use task_meta::{TaskMeta, TaskOperation};

    use super::{AlienResult, OnceGet};
    use crate::CoreFunction;

    static CORE_FUNC: Once<Box<dyn CoreFunction>> = Once::new();

    extern "C" {
        fn sbss();
        fn ebss();
    }
    fn clear_bss() {
        unsafe {
            core::slice::from_raw_parts_mut(
                sbss as usize as *mut u8,
                ebss as usize - sbss as usize,
            )
            .fill(0);
        }
    }

    pub fn init(syscall: Box<dyn CoreFunction>) {
        clear_bss();
        CORE_FUNC.call_once(|| syscall);
    }

    pub fn alloc_raw_pages(n: usize, domain_id: u64) -> *mut u8 {
        CORE_FUNC.get_must().sys_alloc_pages(domain_id, n)
    }

    pub fn free_raw_pages(p: *mut u8, n: usize, domain_id: u64) {
        CORE_FUNC.get_must().sys_free_pages(domain_id, p, n);
    }

    pub fn write_console(s: &str) {
        CORE_FUNC.get_must().sys_write_console(s);
    }

    pub fn backtrace(domain_id: u64) {
        CORE_FUNC.get_must().sys_backtrace(domain_id);
    }

    pub fn trampoline_addr() -> usize {
        static TRAMPOLINE_ADDR: Once<usize> = Once::new();

        TRAMPOLINE_ADDR.call_once(|| CORE_FUNC.get_must().sys_trampoline_addr());
        *TRAMPOLINE_ADDR.get_must()
    }

    pub fn kernel_satp() -> usize {
        static SATP: Once<usize> = Once::new();

        SATP.call_once(|| CORE_FUNC.get_must().sys_kernel_satp());
        *SATP.get_must()
    }

    pub fn trap_from_user() -> usize {
        static TRAP_FROM_USER: Once<usize> = Once::new();

        TRAP_FROM_USER.call_once(|| CORE_FUNC.get_must().sys_trap_from_user());
        *TRAP_FROM_USER.get_must()
    }

    pub fn trap_to_user() -> usize {
        static TRAP_TO_USER: Once<usize> = Once::new();

        TRAP_TO_USER.call_once(|| CORE_FUNC.get_must().sys_trap_to_user());
        *TRAP_TO_USER.get_must()
    }

    // todo!(delete)
    pub fn blk_crash_trick() -> bool {
        CORE_FUNC.get_must().blk_crash_trick()
    }

    pub fn get_domain(name: &str) -> Option<DomainType> {
        CORE_FUNC.get_must().sys_get_domain(name)
    }

    pub fn create_domain(identifier: &str) -> Option<DomainType> {
        CORE_FUNC.get_must().sys_create_domain(identifier)
    }

    pub fn register_domain(ident: &str, ty: DomainTypeRaw, data: &[u8]) -> AlienResult<()> {
        CORE_FUNC.get_must().sys_register_domain(ident, ty, data)
    }

    pub fn update_domain(
        old_domain_name: &str,
        new_domain_name: &str,
        ty: DomainTypeRaw,
    ) -> AlienResult<()> {
        CORE_FUNC
            .get_must()
            .sys_update_domain(old_domain_name, new_domain_name, ty)
    }

    pub fn reload_domain(domain_name: &str) -> AlienResult<()> {
        CORE_FUNC.get_must().sys_reload_domain(domain_name)
    }
    pub fn vaddr_to_paddr_in_kernel(vaddr: usize) -> AlienResult<usize> {
        CORE_FUNC.get_must().vaddr_to_paddr_in_kernel(vaddr)
    }

    pub fn current_tid() -> AlienResult<Option<usize>> {
        CORE_FUNC
            .get_must()
            .task_op(TaskOperation::Current)
            .map(|res| res.current_tid())
    }
    /// return kstack top
    pub fn add_one_task(task_meta: TaskMeta) -> AlienResult<usize> {
        CORE_FUNC
            .get_must()
            .task_op(TaskOperation::Create(task_meta))
            .map(|res| res.kstack_top())
    }
    /// Set current task to wait and switch to next task
    pub fn wait_now() -> AlienResult<()> {
        CORE_FUNC.get_must().task_op(TaskOperation::Wait)?;
        Ok(())
    }
    /// Wake up the task with tid
    pub fn wake_up_wait_task(tid: usize) -> AlienResult<()> {
        CORE_FUNC.get_must().task_op(TaskOperation::Wakeup(tid))?;
        Ok(())
    }
    /// Yield the current task
    pub fn yield_now() -> AlienResult<()> {
        CORE_FUNC.get_must().task_op(TaskOperation::Yield)?;
        Ok(())
    }
    pub fn exit_now() -> AlienResult<()> {
        CORE_FUNC.get_must().task_op(TaskOperation::Exit)?;
        Ok(())
    }
    /// remove task from scheduler, release resources
    pub fn remove_task(tid: usize) -> AlienResult<()> {
        CORE_FUNC.get_must().task_op(TaskOperation::Remove(tid))?;
        Ok(())
    }

    /// Check if the task is exit over
    pub fn is_task_exit(tid: usize) -> AlienResult<bool> {
        CORE_FUNC
            .get_must()
            .task_op(TaskOperation::ExitOver(tid))
            .map(|res| res.is_exit_over())
    }
}

impl<T> OnceGet<T> for Once<T> {
    fn get_must(&self) -> &T {
        unsafe { self.get_unchecked() }
    }
}

pub trait OnceGet<T> {
    fn get_must(&self) -> &T;
}
