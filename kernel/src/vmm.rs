use crate::address::{PhysicalAddr, VirtualAddr};

pub struct Vmm {}

impl Vmm {
    pub fn new() -> Self {
        Self {}
    }

    pub fn map_memory(&mut self, paddr: PhysicalAddr, vaddr: VirtualAddr) {}
}
