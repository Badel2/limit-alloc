use std::alloc::{GlobalAlloc, Layout};
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct Limit<A> {
    limit: usize,
    alloc: A,
}

impl<A: GlobalAlloc> Limit<A> {
    pub const fn new(limit: usize, alloc: A) -> Self {
        Self { limit, alloc }
    }

    /// Returns None if the memory limit would be exhausted after allocating.
    ///
    /// # Safety
    ///
    /// The same restrictions as `GlobalAlloc::alloc`.
    pub unsafe fn try_alloc(&self, layout: Layout) -> Option<*mut u8> {
        match ALLOCATED.fetch_update(SeqCst, SeqCst, |old| {
            let new = old.checked_add(layout.size())?;
            if new > self.limit {
                None
            } else {
                Some(new)
            }
        }) {
            Ok(_size) => {}
            Err(_e) => return None,
        }
        let ret = self.alloc.alloc(layout);
        if ret.is_null() {
            // Nothing was actually allocated, so subtract the size
            ALLOCATED.fetch_sub(layout.size(), SeqCst);
        }

        Some(ret)
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for Limit<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.try_alloc(layout).unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.alloc.dealloc(ptr, layout);
        ALLOCATED.fetch_sub(layout.size(), SeqCst);
    }
}
