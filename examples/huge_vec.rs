use limit_alloc::Limit;
use std::alloc::System;

// Limit available RAM to 4MB
#[global_allocator]
static A: Limit<System> = Limit::new(4_000_000, System);

fn main() {
    let _huge_vec: Vec<u8> = Vec::with_capacity(4_000_001);
}
