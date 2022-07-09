//! Allocator that allows to limit the available memory.
//!
//! This crate implements a few similar types, you can choose the best depending on your use case:
//!
//! * Use `ConstLimit` if you know the limit at compile time, because that makes the allocator
//! zero-sized (as long as the inner allocator is also zero-sized).
//! * Use `Limit` if you are not sure, or if you need more than one limit in the same application.
//! This is needed because `ConstLimit` uses a static counter to store the allocated memory, so it
//! is impossible to track the memory allocated by different instances of the allocator, we can
//! only track the total allocated memory. The size of `Limit` is `1 * usize`.
//! * Use `ArcLimit` if you need a `Limit` that implements `Clone`. Ideally you would have been
//! able to use `Arc<Limit<A>>` instead, but `Arc<T>` cannot implement `GlobalAlloc`.
//!
//! Note on alignment: an allocation of 1 byte with alignment greater than 1, for example 2 bytes,
//! will allocate 2 bytes because of padding. But this crate only counts 1 byte. So the limit may
//! not be completely accurate.
use std::alloc::{GlobalAlloc, Layout};
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

pub struct Limit<A> {
    remaining: AtomicUsize,
    alloc: A,
}

impl<A: GlobalAlloc> Limit<A> {
    pub const fn new(limit: usize, alloc: A) -> Self {
        Self {
            remaining: AtomicUsize::new(limit),
            alloc,
        }
    }

    /// Returns None if the memory limit would be exhausted after allocating.
    ///
    /// # Safety
    ///
    /// The same restrictions as `GlobalAlloc::alloc`.
    pub unsafe fn try_alloc(&self, layout: Layout) -> Option<*mut u8> {
        match self
            .remaining
            .fetch_update(SeqCst, SeqCst, |old| old.checked_sub(layout.size()))
        {
            Ok(_size) => {}
            Err(_e) => return None,
        }
        let ret = self.alloc.alloc(layout);
        if ret.is_null() {
            // Nothing was actually allocated, so add back the size
            self.remaining.fetch_add(layout.size(), SeqCst);
        }

        Some(ret)
    }

    /// Returns remaining memory in bytes. This value does not guarantee that an allocation of x
    /// bytes will succeed.
    pub fn remaining(&self) -> usize {
        self.remaining.load(SeqCst)
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for Limit<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.try_alloc(layout).unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.alloc.dealloc(ptr, layout);
        self.remaining.fetch_add(layout.size(), SeqCst);
    }
}

unsafe impl<'a, A: GlobalAlloc> GlobalAlloc for &'a Limit<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Limit::alloc(self, layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Limit::dealloc(self, ptr, layout)
    }
}

pub struct ArcLimit<A>(Arc<Limit<A>>);

impl<A> Clone for ArcLimit<A> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<A: GlobalAlloc> ArcLimit<A> {
    pub fn new(l: Limit<A>) -> Self {
        Self(Arc::new(l))
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for ArcLimit<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Limit::alloc(&self.0, layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Limit::dealloc(&self.0, ptr, layout)
    }
}

/// Total memory allocated by `ConstLimit`, in bytes.
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct ConstLimit<A, const L: usize> {
    alloc: A,
}

impl<A: GlobalAlloc, const L: usize> ConstLimit<A, L> {
    pub const fn new(alloc: A) -> Self {
        Self { alloc }
    }

    /// Returns None if the memory limit would be exhausted after allocating.
    ///
    /// # Safety
    ///
    /// The same restrictions as `GlobalAlloc::alloc`.
    pub unsafe fn try_alloc(&self, layout: Layout) -> Option<*mut u8> {
        match ALLOCATED.fetch_update(SeqCst, SeqCst, |old| {
            let new = old.checked_add(layout.size())?;
            if new > L {
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

    /// Returns remaining memory in bytes. This value does not guarantee that an allocation of x
    /// bytes will succeed.
    pub fn remaining(&self) -> usize {
        L.checked_sub(ALLOCATED.load(SeqCst))
            .expect("bug: allocated more memory than the limit")
    }
}

unsafe impl<A: GlobalAlloc, const L: usize> GlobalAlloc for ConstLimit<A, L> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.try_alloc(layout).unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.alloc.dealloc(ptr, layout);
        ALLOCATED.fetch_sub(layout.size(), SeqCst);
    }
}
