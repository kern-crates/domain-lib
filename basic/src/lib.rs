#![no_std]
#[macro_use]
pub mod console;
pub mod arch;
pub mod bus;
pub mod config;
pub mod io;
#[cfg(feature = "log")]
pub mod logging;
pub mod sync;
#[cfg(feature = "task")]
pub mod task;
pub mod time;
pub mod vm;

extern crate alloc;

pub use corelib::{
    add_one_task, backtrace, blk_crash_trick, constants, create_domain, current_tid, exit_now,
    get_domain, is_task_exit, kernel_satp, register_domain, reload_domain, remove_task,
    trap_from_user, trap_to_user, update_domain, vaddr_to_paddr_in_kernel, wait_now,
    wake_up_wait_task, write_console, yield_now, AlienError, AlienResult,
};
