#![feature(allocator_api)]
#![no_std]
#![no_main]
extern crate alloc;
use alloc::{boxed::Box, sync::Arc};
use core::{
    alloc::{AllocError, Allocator, Layout},
    any::Any,
    ptr::NonNull,
};

use spin::Once;

pub trait SendAllocator: Allocator + Send + Sync {}
pub trait DomainDataStorage: Send + Sync {
    fn insert(
        &self,
        key: &str,
        value: Box<Arc<dyn Any + Send + Sync, DataStorageHeap>, DataStorageHeap>,
    ) -> Option<Box<Arc<dyn Any + Send + Sync, DataStorageHeap>, DataStorageHeap>>;
    fn get(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync, DataStorageHeap>>;
    fn remove(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync, DataStorageHeap>>;
}

#[derive(Clone)]
pub struct DataStorageHeap;

unsafe impl Allocator for DataStorageHeap {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        DATA_ALLOCATOR
            .get()
            .expect("allocate: data allocator not initialized")
            .allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        DATA_ALLOCATOR
            .get()
            .expect("deallocate: data allocator not initialized")
            .deallocate(ptr, layout)
    }
}

static DATA_ALLOCATOR: Once<Box<dyn SendAllocator>> = Once::new();

pub fn init_data_allocator(allocator: Box<dyn SendAllocator>) {
    DATA_ALLOCATOR.call_once(|| allocator);
    log::error!("init data allocator success");
}

#[allow(dead_code)]
pub struct StorageArg {
    pub allocator: Box<dyn SendAllocator>,
    pub storage: Box<dyn DomainDataStorage>,
}

impl StorageArg {
    pub fn new(allocator: Box<dyn SendAllocator>, storage: Box<dyn DomainDataStorage>) -> Self {
        Self { allocator, storage }
    }
}

#[cfg(feature = "impl")]
mod __private {
    use alloc::{boxed::Box, sync::Arc};
    use core::any::Any;

    use spin::Once;

    use crate::{DataStorageHeap, DomainDataStorage};

    pub fn insert_data<T: Any + Send + Sync>(
        key: &str,
        value: T,
    ) -> Option<Arc<T, DataStorageHeap>> {
        let arc = Arc::new_in(value, DataStorageHeap);
        let arc_wrapper: Box<Arc<dyn Any + Send + Sync, DataStorageHeap>, DataStorageHeap> =
            Box::new_in(arc, DataStorageHeap);
        let old = DATABASE.get().unwrap().insert(key, arc_wrapper);
        let old_arc = match old {
            Some(old) => {
                let arc = *old;
                arc.downcast::<T>().ok()
            }
            None => None,
        };
        old_arc
    }

    pub fn get_data<T: Any + Send + Sync>(key: &str) -> Option<Arc<T, DataStorageHeap>> {
        let res = DATABASE.get().unwrap().get(key).and_then(|arc_wrapper| {
            let res = arc_wrapper.downcast::<T>().ok();
            res
        });
        res
    }

    pub fn get_or_insert_with_data<T: Any + Send + Sync, F: FnOnce() -> T>(
        key: &str,
        f: F,
    ) -> Arc<T, DataStorageHeap> {
        let arc = get_data::<T>(&key);
        match arc {
            Some(arc) => arc,
            None => {
                let value = f();
                let arc = Arc::new_in(value, DataStorageHeap);
                let arc_wrapper: Box<Arc<dyn Any + Send + Sync, DataStorageHeap>, DataStorageHeap> =
                    Box::new_in(arc.clone(), DataStorageHeap);
                DATABASE.get().unwrap().insert(key, arc_wrapper);
                arc
            }
        }
    }

    pub fn remove_data<T: Any + Send + Sync>(key: &str) -> Option<Arc<T, DataStorageHeap>> {
        let res = DATABASE.get().unwrap().remove(key).and_then(|arc_wrapper| {
            let res = arc_wrapper.downcast::<T>().ok();
            res
        });
        res
    }

    static DATABASE: Once<Box<dyn DomainDataStorage>> = Once::new();

    pub fn init_database(database: Box<dyn DomainDataStorage>) {
        DATABASE.call_once(|| database);
        log::error!("init database success");
    }
}

#[cfg(feature = "impl")]
pub use __private::*;
