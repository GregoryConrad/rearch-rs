<p align="center">
<a href="https://github.com/GregoryConrad/rearch-rs/actions"><img src="https://github.com/GregoryConrad/rearch-rs/actions/workflows/test.yml/badge.svg" alt="CI Status"></a>
<a href="https://github.com/GregoryConrad/rearch-rs"><img src="https://img.shields.io/github/stars/GregoryConrad/rearch-rs.svg?style=flat&logo=github&colorB=deeppink&label=stars" alt="Github Stars"></a>
<a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-purple.svg" alt="MIT License"></a>
</p>

<p align="center">
<img src="https://github.com/GregoryConrad/rearch-docs/blob/main/assets/banner.jpg?raw=true" width="75%" alt="Banner" />
</p>

<p align="center">
rearch = re-imagined approach to application design and architecture
</p>

---


## Features
Specifically, rearch is a:
- ⚡️ Reactive
- 🧮 Functional
- 🔍 Testable
- 🧱 Composable
- 🔌 Extendable
- ⬆️ Scalable
- 💉 Dependency Injection

Framework.


# Under Construction
This README is a large WIP, but there are some basics here.


## Getting Started
First, you need to define some "capsules."

There are two ways to define capsules, depending on your toolchain.
Both are 100% compatible with each other, and will continue to be in the future as well
(forward compatability was a *very* strong factor when designing the api).

### Nightly Rust (`better-api` feature)
Once `unboxed_closures` and `fn_traits` stabilize,
this nightly syntax will be the preferred syntax (over the current stable syntax),
and this will no longer be feature-gated.
```rust
fn count(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = register(side_effects::state(0));
    (*state, set_state)
}

fn count_plus_one(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
    get(count).0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```

### Stable Rust
Once `unboxed_closures` and `fn_traits` stabilize, the below will be deprecated in favor
of the now nightly-only syntax (backward compatability will be maintained).
```rust
fn count(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl Fn(u8) + Clone + Send + Sync) {
    let (state, set_state) = register.register(side_effects::state(0));
    (*state, set_state)
}

fn count_plus_one(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
    get.get(count).0 + 1
}

let container = Container::new();
let ((count, set_count), count_plus_one) = container.read((count, count_plus_one));
```
