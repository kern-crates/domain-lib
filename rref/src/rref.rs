//! RRef is a reference counted reference type that is used to share data between domains.
//!
//! Reference: https://std-dev-guide.rust-lang.org/policy/specialization.html
use alloc::collections::BTreeMap;
use core::{
    alloc::Layout,
    any::TypeId,
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut},
};

use spin::Mutex;

use super::{CustomDrop, RRefable, SharedData, TypeIdentifiable};

#[repr(C)]
pub struct RRef<T>
where
    T: 'static + RRefable,
{
    domain_id_pointer: *mut u64,
    borrow_count_pointer: *mut u64,
    value_pointer: *mut T,
}

unsafe impl<T: RRefable> RRefable for RRef<T> {}
unsafe impl<T: RRefable> Send for RRef<T> where T: Send {}
unsafe impl<T: RRefable> Sync for RRef<T> where T: Sync {}

pub fn drop_no_type<T: CustomDrop>(ptr: *mut u8) {
    let ptr = ptr as *mut T;
    unsafe { &mut *ptr }.custom_drop();
}

static DROP: Mutex<BTreeMap<TypeId, fn(ptr: *mut u8)>> = Mutex::new(BTreeMap::new());

pub fn drop_domain_share_data(id: TypeId, ptr: *mut u8) {
    let drop = DROP.lock();
    let drop_fn = drop.get(&id).unwrap();
    drop_fn(ptr);
}

impl<T: RRefable> RRef<T>
where
    T: TypeIdentifiable,
{
    pub unsafe fn new_with_layout(value: T, layout: Layout) -> RRef<T> {
        let type_id = T::type_id();
        let mut drop_guard = DROP.lock();
        if !drop_guard.contains_key(&type_id) {
            drop_guard.insert(type_id, drop_no_type::<T>);
        }
        drop(drop_guard);

        let allocation = match crate::share_heap_alloc(layout, type_id, drop_domain_share_data) {
            Some(allocation) => allocation,
            None => panic!("Shared heap allocation failed"),
        };
        let value_pointer = allocation.value_pointer as *mut T;
        *allocation.domain_id_pointer = crate::domain_id();
        *allocation.borrow_count_pointer = 0;
        core::ptr::write(value_pointer, value);
        RRef {
            domain_id_pointer: allocation.domain_id_pointer,
            borrow_count_pointer: allocation.borrow_count_pointer,
            value_pointer,
        }
    }
    pub fn new(value: T) -> RRef<T> {
        let layout = Layout::new::<T>();
        unsafe { Self::new_with_layout(value, layout) }
    }
    pub fn new_aligned(value: T, align: usize) -> RRef<T> {
        let size = core::mem::size_of::<T>();
        let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
        unsafe { Self::new_with_layout(value, layout) }
    }
    pub fn domain_id(&self) -> u64 {
        unsafe { *self.domain_id_pointer }
    }
}

impl<T: RRefable> Deref for RRef<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.value_pointer }
    }
}

impl<T: RRefable> DerefMut for RRef<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value_pointer }
    }
}

impl<T: RRefable> Drop for RRef<T> {
    fn drop(&mut self) {
        log::warn!("<drop> for RRef {:#x}", self.value_pointer as usize);
        self.custom_drop();
    }
}

impl<T: RRefable> CustomDrop for RRef<T> {
    fn custom_drop(&mut self) {
        log::warn!("<custom_drop> for RRef {:#x}", self.value_pointer as usize);
        let value = unsafe { &mut *self.value_pointer };
        value.custom_drop();
        crate::share_heap_dealloc(self.value_pointer as *mut u8);
    }
}

impl<T: RRefable + Debug> Debug for RRef<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let value = unsafe { &*self.value_pointer };
        let domain_id = unsafe { *self.domain_id_pointer };
        let borrow_count = unsafe { *self.borrow_count_pointer };
        f.debug_struct("RRef")
            .field("value", value)
            .field("domain_id", &domain_id)
            .field("borrow_count", &borrow_count)
            .finish()
    }
}

impl<T: RRefable> SharedData for RRef<T> {
    fn move_to(&self, new_domain_id: u64) -> u64 {
        unsafe {
            let old_domain_id = *self.domain_id_pointer;
            *self.domain_id_pointer = new_domain_id;
            old_domain_id
        }
    }
}
