use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};

use spin::Mutex;

use crate::{
    sheap::{free_domain_shared_data, FreeShared},
    storage_heap::DomainDataMap,
    FRAME_BITS,
};

pub static DOMAIN_RESOURCE: Mutex<DomainResource> = Mutex::new(DomainResource::new());
pub struct DomainResource {
    page_map: BTreeMap<u64, Vec<(usize, usize)>>,
    box_data: BTreeMap<u64, usize>,
}

impl DomainResource {
    pub const fn new() -> Self {
        Self {
            page_map: BTreeMap::new(),
            box_data: BTreeMap::new(),
        }
    }

    pub fn insert_page_map(&mut self, domain_id: u64, page: (usize, usize)) {
        let vec = self.page_map.entry(domain_id).or_default();
        vec.push(page);
    }

    pub fn free_page_map(&mut self, domain_id: u64, page: usize) {
        let vec = self.page_map.get_mut(&domain_id).unwrap();
        vec.retain(|(s, _)| *s != page);
    }

    pub fn insert_box_data(&mut self, domain_id: u64, data: usize) {
        self.box_data.insert(domain_id, data);
    }
}

pub fn register_domain_resource(domain_id: u64, box_ptr: usize) {
    DOMAIN_RESOURCE.lock().insert_box_data(domain_id, box_ptr);
}

pub trait FreeFrame {
    fn free_frame(addr: *mut u8, num: usize);
}

pub fn free_domain_resource<T>(domain_id: u64, free_shared: FreeShared, free: T)
where
    T: Fn(*mut u8, usize),
{
    // println!("free_domain_resource for domain_id: {}", domain_id);

    // free shared data
    free_domain_shared_data(domain_id, free_shared);

    let mut binding = DOMAIN_RESOURCE.lock();
    // free pages
    if let Some(vec) = binding.page_map.remove(&domain_id) {
        for (page_start, n) in vec {
            let page_end = page_start + n;
            log::warn!(
                "[Domain: {}] free pages: [{:#x}-{:#x}]",
                domain_id,
                page_start << FRAME_BITS,
                page_end << FRAME_BITS
            );
            free((page_start << FRAME_BITS) as *mut u8, n);
        }
    }

    // free Box<DomainDataMap>
    let ptr = binding.box_data.remove(&domain_id);
    if let Some(data_map_addr) = ptr {
        let data_map = unsafe { Box::from_raw(data_map_addr as *mut DomainDataMap) };
        drop(data_map);
        // println_color!(31, "[Domain: {}] free DomainDataMap resource", domain_id);
    }
}
