use core::{
    ops::{Deref, DerefMut, Range},
    sync::atomic::AtomicUsize,
};

use config::FRAME_SIZE;
use memory_addr::{PhysAddr, VirtAddr};
use shared_heap::domain_id;

#[derive(Debug)]
pub struct FrameTracker {
    ptr: usize,
    page_count: usize,
    // should be deallocated
    dealloc: bool,
}

impl FrameTracker {
    /// Allocate `page_count` pages and return a `FrameTracker` pointing to the start of the allocated memory.
    pub fn new(page_count: usize) -> Self {
        let ptr = corelib::alloc_raw_pages(page_count, domain_id()) as usize;
        Self {
            ptr,
            page_count,
            dealloc: true,
        }
    }

    pub fn create_trampoline() -> Self {
        let trampoline_phy_addr = corelib::trampoline_addr();
        Self {
            ptr: trampoline_phy_addr,
            page_count: 1,
            dealloc: false,
        }
    }

    pub fn from_phy_range(r: Range<usize>) -> Self {
        assert_eq!(r.start % FRAME_SIZE, 0);
        assert_eq!(r.end % FRAME_SIZE, 0);
        Self {
            ptr: r.start,
            page_count: (r.end - r.start) / FRAME_SIZE,
            dealloc: false,
        }
    }

    /// Return the physical address of the start of the frame.
    pub fn start_phy_addr(&self) -> PhysAddr {
        PhysAddr::from(self.ptr)
    }

    /// Return the virtual address of the start of the frame.
    pub fn start_virt_addr(&self) -> VirtAddr {
        VirtAddr::from(self.ptr)
    }

    /// Return the physical address of the end of the frame.
    pub fn end_phy_addr(&self) -> PhysAddr {
        PhysAddr::from(self.end())
    }

    /// Return the virtual address of the end of the frame.
    pub fn end_virt_addr(&self) -> VirtAddr {
        VirtAddr::from(self.end())
    }

    fn end(&self) -> usize {
        self.ptr + self.size()
    }

    pub fn size(&self) -> usize {
        self.page_count * FRAME_SIZE
    }

    pub fn clear(&self) {
        unsafe {
            core::ptr::write_bytes(self.ptr as *mut u8, 0, self.size());
        }
    }
    pub fn as_mut_slice_with<'a, T>(&self, offset: usize) -> &'a mut [T] {
        let t_size = core::mem::size_of::<T>();
        assert_eq!((self.size() - offset) % t_size, 0);
        let ptr = self.ptr + offset;
        unsafe { core::slice::from_raw_parts_mut(ptr as *mut T, (self.size() - offset) / t_size) }
    }
    pub fn as_slice_with<'a, T>(&self, offset: usize) -> &'a [T] {
        let t_size = core::mem::size_of::<T>();
        assert_eq!((self.size() - offset) % t_size, 0);
        let ptr = self.ptr + offset;
        unsafe { core::slice::from_raw_parts(ptr as *const T, (self.size() - offset) / t_size) }
    }

    pub fn as_mut_with<'a, T: Sized>(&self, offset: usize) -> &'a mut T {
        assert!(offset + core::mem::size_of::<T>() <= self.size());
        let ptr = self.ptr + offset;
        unsafe { &mut *(ptr as *mut T) }
    }

    pub fn as_with<'a, T: Sized>(&self, offset: usize) -> &'a T {
        assert!(offset + core::mem::size_of::<T>() <= self.size());
        let ptr = self.ptr + offset;
        unsafe { &*(ptr as *const T) }
    }

    pub fn read_value_atomic(&self, offset: usize) -> usize {
        assert!(offset + core::mem::size_of::<usize>() <= self.size());
        let ptr = self.ptr + offset;
        unsafe {
            AtomicUsize::from_ptr(ptr as *mut usize).load(core::sync::atomic::Ordering::SeqCst)
        }
    }

    pub fn write_value_atomic(&self, offset: usize, value: usize) {
        assert!(offset + core::mem::size_of::<usize>() <= self.size());
        let ptr = self.ptr + offset;
        unsafe {
            AtomicUsize::from_ptr(ptr as *mut usize)
                .store(value, core::sync::atomic::Ordering::SeqCst)
        }
    }
}

impl Deref for FrameTracker {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr as *const u8, self.size()) }
    }
}

impl DerefMut for FrameTracker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut u8, self.size()) }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        if self.dealloc {
            corelib::free_raw_pages(self.ptr as *mut u8, self.page_count, domain_id());
        }
    }
}
