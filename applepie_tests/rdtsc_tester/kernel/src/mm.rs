use core::sync::atomic::{AtomicUsize, Ordering};
use core::alloc::{GlobalAlloc, Layout};

static ALLOC_BASE: AtomicUsize = AtomicUsize::new(0x00100000);
const ALLOC_END: usize = 0x180000;

pub struct GlobalAllocator {}

unsafe impl GlobalAlloc for GlobalAllocator {
    /// Allocate virtual memory on this core according to `layout`
    ///
    /// This is a page heap allocator. Pages are allocated from physical memory
    /// and commit to a random address as RW.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = if layout.size() > 0 { layout.size() } else { 1 };

        // 64-byte align size
        let size = (size + 0x3f) & !0x3f;

        let base = ALLOC_BASE.fetch_add(size, Ordering::SeqCst);
        assert!(base + size <= ALLOC_END, "Out of memory");

        base as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Best free implementation 2018
    }
}

