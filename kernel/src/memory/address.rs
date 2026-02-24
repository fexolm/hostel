use core::fmt::Display;

use crate::memory::errors::{MemoryError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalAddr(u64);

impl PhysicalAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr & !0xFFF)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn add(self, offset: u64) -> PhysicalAddr {
        PhysicalAddr(self.0 + offset)
    }

    pub const fn align_up(self, align: u64) -> PhysicalAddr {
        assert!(align.is_power_of_two());
        PhysicalAddr((self.0 + align - 1) & !(align - 1))
    }

    pub const fn to_virtual(self) -> Result<VirtualAddr> {
        if self.0 > crate::memory::constants::MAX_PHYSICAL_ADDR {
            Err(MemoryError::PhysicalToVirtual { addr: self.0 })
        } else {
            Ok(VirtualAddr(
                self.0 + crate::memory::constants::DIRECT_MAP_OFFSET.0,
            ))
        }
    }
}

impl Display for PhysicalAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualAddr(u64);

impl VirtualAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn add(self, offset: u64) -> VirtualAddr {
        VirtualAddr(self.0 + offset)
    }

    pub const fn to_physical(self) -> Result<PhysicalAddr> {
        if self.0 < crate::memory::constants::DIRECT_MAP_OFFSET.0
            || self.0
                > crate::memory::constants::DIRECT_MAP_OFFSET.0
                    + crate::memory::constants::MAX_PHYSICAL_ADDR
        {
            Err(MemoryError::VirtualToPhysical { addr: self.0 })
        } else {
            Ok(PhysicalAddr(
                self.0 - crate::memory::constants::DIRECT_MAP_OFFSET.0,
            ))
        }
    }

    pub const fn pml4_index(self) -> u64 {
        (self.0 >> 39) & 0x1FF
    }

    pub const fn pdpt_index(self) -> u64 {
        (self.0 >> 30) & 0x1FF
    }

    pub const fn pd_index(self) -> u64 {
        (self.0 >> 21) & 0x1FF
    }

    pub const fn as_ptr<T>(self) -> *mut T {
        self.0 as usize as *mut T
    }

    pub unsafe fn as_ref_mut<'i, T>(self) -> &'i mut T {
        debug_assert!(self.0 as usize % core::mem::align_of::<T>() == 0);
        unsafe { &mut *self.as_ptr() }
    }
}

impl Display for VirtualAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}
