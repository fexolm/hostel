#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalAddr(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualAddr(pub u64);

impl VirtualAddr {
    pub const fn add(self, offset: u64) -> VirtualAddr {
        VirtualAddr(self.0 + offset)
    }
}
