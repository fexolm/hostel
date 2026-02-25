use thiserror::Error as ThisError;

#[derive(ThisError, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    #[error("failed to convert virtual address {addr:#x} to physical")]
    VirtualToPhysical { addr: usize },

    #[error("failed to convert physical address {addr:#x} to virtual")]
    PhysicalToVirtual { addr: usize },

    #[error("invalid page count: {pages}")]
    InvalidPageCount { pages: usize },

    #[error("out of memory")]
    OutOfMemory,

    #[error("virtual address {addr:#x} is already mapped")]
    AlreadyMapped { addr: usize },

    #[error("pointer {addr:#x} is not in direct-map region")]
    PointerNotInDirectMap { addr: usize },

    #[error("allocation too large: requested {requested} bytes, max {max} bytes")]
    AllocationTooLarge { requested: usize, max: usize },

    #[error("too many slabs for class {class_size}")]
    TooManySlabs { class_size: u32 },

    #[error("too many active large allocations")]
    TooManyLargeAllocations,

    #[error("unknown allocation at physical address {addr:#x}")]
    UnknownAllocation { addr: usize },

    #[error("pointer {addr:#x} does not match slab alignment {block_size}")]
    SlabAlignmentMismatch { addr: usize, block_size: usize },

    #[error("invalid slab capacity")]
    InvalidSlabCapacity,

    #[error("slab is empty")]
    SlabEmpty,

    #[error("page refcount overflow at physical address {addr:#x}")]
    PageRefcountOverflow { addr: usize },
}

pub type Result<T> = core::result::Result<T, MemoryError>;
