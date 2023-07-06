# rearch-rs

The README is a large WIP, but below are some basics:

## Getting Started
First, you need to define some "capsules."
There are several ways to define capsules, depending on your toolchain and opinions toward macros.

### Macros (Optional), Stable and Nightly Rust
Note: This particular example uses the nightly-only `better-api` feature.
You can also write macros using the stable Rust syntax further below.
```rust
#[capsule]
fn count(register: SideEffectRegistrar) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = register(side_effects::state(0));
    (*state, set_state)
}

#[capsule]
fn count_plus_one() -> u8 {
  $count.0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```

### Vanilla, Nightly Rust (`better-api` feature)
```rust
fn count(
    _: CapsuleReader, register: SideEffectRegistrar,
) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = register(side_effects::state(0));
    (*state, set_state)
}

fn count_plus_one(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
  get(count).0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```

### Vanilla, Stable Rust
```rust
fn count(
    _: CapsuleReader, registrar: SideEffectRegistrar,
) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = registrar.register(side_effects::state(0));
    (*state, set_state)
}

fn count_plus_one(mut reader: CapsuleReader, _: SideEffectRegistrar) -> u8 {
  reader.read(count).0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```
