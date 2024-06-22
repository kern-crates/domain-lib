#![feature(allocator_api)]
#![feature(btreemap_alloc)]
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

#[derive(Clone)]
pub struct DataStorageHeap;

unsafe impl Allocator for DataStorageHeap {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        DATA_ALLOCATOR.get().unwrap().allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        DATA_ALLOCATOR.get().unwrap().deallocate(ptr, layout)
    }
}

pub trait DomainDataStorage: Send + Sync {
    fn insert(
        &self,
        key: &str,
        value: Box<dyn Any + Send + Sync, DataStorageHeap>,
    ) -> Option<Box<dyn Any + Send + Sync, DataStorageHeap>>;
    fn get(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync, DataStorageHeap>>;
}

pub fn insert_data<T: Any + Send + Sync>(key: &str, value: T) -> Option<T> {
    let arc_wrapper = Box::new_in(Arc::new_in(value, DataStorageHeap), DataStorageHeap);
    let old = DATABASE.get().unwrap().insert(key, arc_wrapper);
    let old_arc = match old {
        Some(old) => old
            .downcast_ref::<Arc<T, DataStorageHeap>>()
            .map(Arc::clone),
        None => None,
    };
    old_arc.and_then(|arc| Arc::try_unwrap(arc).ok())
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
            insert_data(key, Box::new_in(arc.clone(), DataStorageHeap));
            arc
        }
    }
}

static DATABASE: Once<Box<dyn DomainDataStorage>> = Once::new();

pub fn init_database(database: Box<dyn DomainDataStorage>) {
    DATABASE.call_once(|| database);
}

static DATA_ALLOCATOR: Once<Box<dyn SendAllocator>> = Once::new();

pub fn init_data_allocator(allocator: Box<dyn SendAllocator>) {
    DATA_ALLOCATOR.call_once(|| allocator);
}
