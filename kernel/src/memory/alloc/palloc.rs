use crate::memory::{
    address::PhysicalAddr,
    constants::{MAX_PHYSICAL_ADDR, PAGE_SIZE, PALLOC_FIRST_PAGE},
    errors::{MemoryError, Result},
};

const BITMAP_SIZE: usize = MAX_PHYSICAL_ADDR / PAGE_SIZE / 64;
const PAGE_COUNT: usize = MAX_PHYSICAL_ADDR / PAGE_SIZE;

#[repr(align(4096))]
#[repr(C)]
struct PageAllocator {
    bitmap: [u64; BITMAP_SIZE],
    refcounts: [u8; PAGE_COUNT],
}

impl PageAllocator {
    const fn new() -> Self {
        let mut bitmap = [0; BITMAP_SIZE];
        let mut refcounts = [0; PAGE_COUNT];
        let mut page = 0;
        let reserved_pages = PALLOC_FIRST_PAGE.as_usize() / PAGE_SIZE;

        while page < reserved_pages {
            let word = page / 64;
            let bit = page % 64;
            bitmap[word] |= 1 << bit;
            refcounts[page] = 1;
            page += 1;
        }

        Self { bitmap, refcounts }
    }

    fn alloc(&mut self, pages: usize) -> Result<PhysicalAddr> {
        if pages == 0 {
            return Err(MemoryError::InvalidPageCount { pages });
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
                let mut used_page = run_start;
                while used_page < run_start + pages {
                    self.refcounts[used_page] = 1;
                    used_page += 1;
                }
                return Ok(PhysicalAddr::new(run_start * PAGE_SIZE));
            }
        }

        Err(MemoryError::OutOfMemory)
    }

    fn free(&mut self, addr: PhysicalAddr) -> Result<()> {
        let page_index = addr.as_usize() / PAGE_SIZE;
        self.refcounts[page_index] -= 1;
        if self.refcounts[page_index] == 0 {
            self.mark_pages(page_index, 1, false);
        }
        Ok(())
    }

    fn share(&mut self, addr: PhysicalAddr) -> Result<()> {
        let page = addr.as_usize() / PAGE_SIZE;
        if self.refcounts[page] == u8::MAX {
            return Err(MemoryError::PageRefcountOverflow {
                addr: page * PAGE_SIZE,
            });
        }

        self.refcounts[page] += 1;
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

static PAGE_ALLOCATOR: spin::Mutex<PageAllocator> = spin::Mutex::new(PageAllocator::new());

pub fn palloc(pages: usize) -> Result<PhysicalAddr> {
    PAGE_ALLOCATOR.lock().alloc(pages)
}

pub fn pfree(addr: PhysicalAddr) -> Result<()> {
    PAGE_ALLOCATOR.lock().free(addr)
}

pub fn pshare(addr: PhysicalAddr) -> Result<()> {
    PAGE_ALLOCATOR.lock().share(addr)
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
        allocator.free(addr1).unwrap();
        let addr3 = allocator.alloc(1).unwrap();
        assert_eq!(addr3, PhysicalAddr::new(first_page)); // should reuse the freed page
    }

    #[test]
    fn test_page_refcount_prevents_early_reuse() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut allocator = PageAllocator::new();
        let base = allocator.alloc(1).unwrap();

        allocator.share(base).unwrap();
        allocator.free(base).unwrap();

        let second = allocator.alloc(1).unwrap();
        assert_ne!(
            base, second,
            "page is still referenced and must not be reused"
        );

        allocator.free(base).unwrap();
        let reused = allocator.alloc(1).unwrap();
        assert_eq!(
            reused, base,
            "page must be reusable after last reference drop"
        );
    }
}
