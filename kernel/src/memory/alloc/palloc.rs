use crate::memory::{
    address::PhysicalAddr,
    constants::{MAX_PHYSICAL_ADDR, PAGE_SIZE, PALLOC_FIRST_PAGE},
    errors::{MemoryError, Result},
};

const BITMAP_SIZE: usize = MAX_PHYSICAL_ADDR / PAGE_SIZE / 64;

#[repr(align(4096))]
#[repr(C)]
struct PageAllocator {
    bitmap: [u64; BITMAP_SIZE],
}

impl PageAllocator {
    const fn new() -> Self {
        let mut bitmap = [0u64; BITMAP_SIZE];
        let mut page = 0usize;
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

        let total_pages = MAX_PHYSICAL_ADDR / PAGE_SIZE;
        let mut run_start = 0usize;
        let mut run_len = 0usize;
        let mut page = 0usize;

        while page < total_pages {
            if self.is_page_used(page) {
                run_len = 0;
                page += 1;
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

            page += 1;
        }

        Err(MemoryError::OutOfMemory)
    }

    fn free(&mut self, addr: PhysicalAddr, pages: usize) -> Result<()> {
        if pages == 0 {
            return Err(MemoryError::InvalidPageCount { pages });
        }

        let page_index = addr.as_usize() / PAGE_SIZE;
        self.mark_pages(page_index, pages, false);
        Ok(())
    }

    fn is_page_used(&self, page_index: usize) -> bool {
        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        (self.bitmap[word_index] & (1 << bit_index)) != 0
    }

    fn mark_pages(&mut self, start_page: usize, pages: usize, used: bool) {
        let mut page = start_page;
        let end = start_page + pages;
        while page < end {
            let word = page / 64;
            let bit = page % 64;
            if used {
                self.bitmap[word] |= 1 << bit;
            } else {
                self.bitmap[word] &= !(1 << bit);
            }
            page += 1;
        }
    }
}

static PAGE_ALLOCATOR: spin::Mutex<PageAllocator> = spin::Mutex::new(PageAllocator::new());

pub fn palloc(pages: usize) -> Result<PhysicalAddr> {
    PAGE_ALLOCATOR.lock().alloc(pages)
}

pub fn pfree(addr: PhysicalAddr, pages: usize) -> Result<()> {
    PAGE_ALLOCATOR.lock().free(addr, pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::alloc::ALLOC_TEST_LOCK;

    #[test]
    fn test_page_allocator() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut allocator = PageAllocator::new();
        let first_page = PALLOC_FIRST_PAGE.as_usize();
        let addr1 = allocator.alloc(1).unwrap();
        let addr2 = allocator.alloc(1).unwrap();
        assert_eq!(addr1, PhysicalAddr::new(first_page));
        assert_eq!(addr2, PhysicalAddr::new(first_page + PAGE_SIZE));
        allocator.free(addr1, 1).unwrap();
        let addr3 = allocator.alloc(1).unwrap();
        assert_eq!(addr3, PhysicalAddr::new(first_page)); // should reuse the freed page
    }
}
