# rearch-rs

The README is a large WIP, but below are some basics.

## Getting Started
First, you need to define some "capsules."

There are several ways to define capsules, depending on your toolchain and opinions toward macros.
Not everyone likes macros, and not everyone can use nightly;
thus, you can pick a combination of the below options that you like,
knowing that every type of capsule syntax shown below are 100% compatible with one another!

### Macros (Optional), Stable and Nightly Rust
Note: This particular example also uses the nightly-only `better-api` feature.
You can also write macros using the stable Rust syntax further below.
```rust
#[capsule]
fn count(register: SideEffectRegistrar) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = register(side_effects::state(0));
    (*state, set_state)
}

#[capsule]
fn count_plus_one() -> u8 {
  _count.0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```

### Vanilla, Nightly Rust (`better-api` feature)
The author's personal favorite syntax; however, the macro is sometimes nice to reduce some churn
(mostly in the capsule function arguments).
Once `unboxed_closures` and `fn_traits` stabilize, the nightly syntax will be the preferred syntax
(over the current stable syntax).
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
Objectively the worst usability and syntax combination of them all.
Once `unboxed_closures` and `fn_traits` stabilize, the below will be deprecated in favor
of the now nightly-only syntax.
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
