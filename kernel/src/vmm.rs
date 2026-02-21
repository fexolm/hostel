use crate::pagetable::PageTableAlloc;

pub struct Vmm {
    pt_alloc: spin::Mutex<PageTableAlloc>,
}
