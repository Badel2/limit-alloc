# limit-alloc

A custom allocator that allows to limit the available memory.

## Usage

```rust
use limit_alloc::Limit;
use std::alloc::System;

// Limit available RAM to 4MB
#[global_allocator]
static A: Limit<System> = Limit::new(4_000_000, System);

fn main() {
    let _huge_vec: Vec<u8> = Vec::with_capacity(4_000_001);
}
```

You can run that example locally and see how the process crashes:

```
$ cargo run --example huge_vec
memory allocation of 4000001 bytes failed
Aborted
```
