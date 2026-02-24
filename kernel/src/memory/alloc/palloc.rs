use crate::memory::{
    address::PhysicalAddr,
    constants::{MAX_PHYSICAL_ADDR, PAGE_SIZE, PALLOC_FIRST_PAGE},
};

const BITMAP_SIZE: usize = (MAX_PHYSICAL_ADDR / PAGE_SIZE / 64) as usize;

#[repr(align(4096))]
#[repr(C)]
struct PageAllocator {
    bitmap: [u64; BITMAP_SIZE],
}

impl PageAllocator {
    const fn new() -> Self {
        let mut bitmap = [0u64; BITMAP_SIZE];
        let mut page = 0u64;
        let reserved_pages = PALLOC_FIRST_PAGE.as_u64() / PAGE_SIZE;

        while page < reserved_pages {
            let word = (page / 64) as usize;
            let bit = page % 64;
            bitmap[word] |= 1 << bit;
            page += 1;
        }

        Self { bitmap }
    }

    fn alloc(&mut self, pages: u64) -> PhysicalAddr {
        assert!(pages > 0);

        let total_pages = MAX_PHYSICAL_ADDR / PAGE_SIZE;
        let mut run_start = 0u64;
        let mut run_len = 0u64;
        let mut page = 0u64;

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
                return PhysicalAddr::new(run_start * PAGE_SIZE);
            }

            page += 1;
        }

        panic!("Out of memory");
    }

    fn free(&mut self, addr: PhysicalAddr, pages: u64) {
        assert!(pages > 0);

        let page_index = addr.as_u64() / PAGE_SIZE;
        self.mark_pages(page_index, pages, false);
    }

    fn is_page_used(&self, page_index: u64) -> bool {
        let word_index = (page_index / 64) as usize;
        let bit_index = page_index % 64;
        (self.bitmap[word_index] & (1 << bit_index)) != 0
    }

    fn mark_pages(&mut self, start_page: u64, pages: u64, used: bool) {
        let mut page = start_page;
        let end = start_page + pages;
        while page < end {
            let word = (page / 64) as usize;
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

pub fn palloc(pages: u64) -> PhysicalAddr {
    PAGE_ALLOCATOR.lock().alloc(pages)
}

pub fn pfree(addr: PhysicalAddr, pages: u64) {
    PAGE_ALLOCATOR.lock().free(addr, pages);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::alloc::ALLOC_TEST_LOCK;

    #[test]
    fn test_page_allocator() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut allocator = PageAllocator::new();
        let first_page = PALLOC_FIRST_PAGE.as_u64();
        let addr1 = allocator.alloc(1);
        let addr2 = allocator.alloc(1);
        assert_eq!(addr1, PhysicalAddr::new(first_page));
        assert_eq!(addr2, PhysicalAddr::new(first_page + PAGE_SIZE));
        allocator.free(addr1, 1);
        let addr3 = allocator.alloc(1);
        assert_eq!(addr3, PhysicalAddr::new(first_page)); // should reuse the freed page
    }
}
