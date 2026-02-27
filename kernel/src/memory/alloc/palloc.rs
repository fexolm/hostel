use crate::memory::{
    address::PhysicalAddr,
    constants::{MAX_PHYSICAL_ADDR, PAGE_SIZE, PALLOC_FIRST_PAGE},
    errors::{MemoryError, Result},
};

const BITMAP_SIZE: usize = MAX_PHYSICAL_ADDR / PAGE_SIZE / 64;
const PAGE_COUNT: usize = MAX_PHYSICAL_ADDR / PAGE_SIZE;

#[repr(align(4096))]
#[repr(C)]
struct PageAllocatorImpl {
    bitmap: [u64; BITMAP_SIZE],
}

impl PageAllocatorImpl {
    const fn new() -> Self {
        let mut bitmap = [0; BITMAP_SIZE];
        let mut page = 0;
        let reserved_pages = PALLOC_FIRST_PAGE.as_usize() / PAGE_SIZE;

        while page < reserved_pages {
            let word = page / 64;
            let bit = page % 64;
            bitmap[word] |= 1 << bit;
            page += 1;
        }

        Self { bitmap }
    }

    fn alloc(&mut self, pages: usize) -> Result<PhysicalAddr> {
        if pages == 0 {
            return Err(MemoryError::InvalidPageCount { pages });
        }

        if pages > PAGE_COUNT {
            return Err(MemoryError::OutOfMemory);
        }

        let mut run_start = 0;
        let mut run_len = 0;

        for page in 0..PAGE_COUNT {
            if self.is_page_used(page) {
                run_len = 0;
                continue;
            }

            if run_len == 0 {
                run_start = page;
            }

            run_len += 1;
            if run_len == pages {
                self.mark_pages(run_start, pages, true);
                return Ok(PhysicalAddr::new(run_start * PAGE_SIZE));
            }
        }

        Err(MemoryError::OutOfMemory)
    }

    fn free(&mut self, addr: PhysicalAddr) -> Result<()> {
        let page_index = addr.as_usize() / PAGE_SIZE;
        self.mark_pages(page_index, 1, false);
        Ok(())
    }

    fn is_page_used(&self, page_index: usize) -> bool {
        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        (self.bitmap[word_index] & (1 << bit_index)) != 0
    }

    fn mark_pages(&mut self, start_page: usize, pages: usize, used: bool) {
        for page in start_page..start_page + pages {
            let word = page / 64;
            let bit = page % 64;
            if used {
                self.bitmap[word] |= 1 << bit;
            } else {
                self.bitmap[word] &= !(1 << bit);
            }
        }
    }
}

fn range_end_page(start_page: usize, pages: usize) -> Result<usize> {
    let end_page = start_page
        .checked_add(pages)
        .ok_or(MemoryError::PhysicalPageOutOfRange { page: start_page })?;

    if start_page >= PAGE_COUNT {
        return Err(MemoryError::PhysicalPageOutOfRange { page: start_page });
    }

    if end_page > PAGE_COUNT {
        return Err(MemoryError::PhysicalPageOutOfRange { page: end_page - 1 });
    }

    Ok(end_page)
}

pub struct PageAllocator(spin::Mutex<PageAllocatorImpl>);

impl PageAllocator {
    pub const fn new() -> Self {
        Self(spin::Mutex::new(PageAllocatorImpl::new()))
    }

    pub fn alloc(&self, pages: usize) -> Result<PhysicalAddr> {
        self.0.lock().alloc(pages)
    }

    pub fn free(&self, addr: PhysicalAddr) -> Result<()> {
        self.0.lock().free(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_allocator() {
        let allocator = Box::new(PageAllocator::new());
        let first_page = PALLOC_FIRST_PAGE.as_usize();
        let addr1 = allocator.alloc(1).unwrap();
        let addr2 = allocator.alloc(1).unwrap();
        assert_eq!(addr1, PhysicalAddr::new(first_page));
        assert_eq!(addr2, PhysicalAddr::new(first_page + PAGE_SIZE));
        allocator.free(addr1).unwrap();
        let addr3 = allocator.alloc(1).unwrap();
        assert_eq!(addr3, PhysicalAddr::new(first_page)); // should reuse the freed page
    }
}
