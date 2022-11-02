# Concurrent Priority Queue

A Priority Queue allows you to prioritize what items come out of the queue based on some predetermined value.

A Concurrent Priority Queue allows you to do this, but it's a `Send + Sync` type with interior mutability (it can be modified without having an exclusive / mutable reference).

See `examples/main` for usage - it's an extremely simple API based on `push` / `pop`.

## Soundness
- `v0.1.0` was an implementation with raw atomics, using Unsafe Rust.
- `v0.2.0` or greater uses a RwLock internally, with zero Unsafe Rust.