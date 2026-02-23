#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalAddr(pub u64);

impl PhysicalAddr {
    pub const fn add(self, offset: u64) -> PhysicalAddr {
        PhysicalAddr(self.0 + offset)
    }

    pub const fn align_up(self, align: u64) -> PhysicalAddr {
        assert!(align.is_power_of_two());
        PhysicalAddr((self.0 + align - 1) & !(align - 1))
    }

    pub const fn to_virtual(self) -> Option<VirtualAddr> {
        if self.0 > crate::constants::MAX_PHYSICAL_ADDR {
            return None;
        }
        Some(VirtualAddr(self.0 + crate::constants::DIRECT_MAP_OFFSET.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualAddr(pub u64);

impl VirtualAddr {
    pub const fn add(self, offset: u64) -> VirtualAddr {
        VirtualAddr(self.0 + offset)
    }

    pub const fn to_physical(self) -> Option<PhysicalAddr> {
        if self.0 < crate::constants::DIRECT_MAP_OFFSET.0
            || self.0 > crate::constants::DIRECT_MAP_OFFSET.0 + crate::constants::MAX_PHYSICAL_ADDR
        {
            return None;
        }
        Some(PhysicalAddr(self.0 - crate::constants::DIRECT_MAP_OFFSET.0))
    }

    pub const fn pml4_index(self) -> u64 {
        (self.0 >> 39) & 0x1FF
    }

    pub const fn pdpt_index(self) -> u64 {
        (self.0 >> 30) & 0x1FF
    }
}
