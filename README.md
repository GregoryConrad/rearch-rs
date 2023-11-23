<p align="center">
<a href="https://github.com/GregoryConrad/rearch-rs/actions"><img src="https://github.com/GregoryConrad/rearch-rs/actions/workflows/test.yml/badge.svg" alt="CI Status"></a>
<a href="https://github.com/GregoryConrad/rearch-rs"><img src="https://img.shields.io/github/stars/GregoryConrad/rearch-rs.svg?style=flat&logo=github&colorB=deeppink&label=stars" alt="Github Stars"></a>
<a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-purple.svg" alt="MIT License"></a>
</p>

<p align="center">
<img src="https://github.com/GregoryConrad/rearch-docs/blob/main/assets/banner.jpg?raw=true" width="75%" alt="Banner" />
</p>

<p align="center">
ReArch = re-imagined approach to application design and architecture
</p>

---


## Features
Specifically, ReArch is a:
- ⚡️ Reactive
- 🧮 Functional
- 🔍 Testable
- 🧱 Composable
- 🔌 Extendable
- ⬆️ Scalable
- 💉 Dependency Injection

Framework.

That's a mouthful! But in short, ReArch is an entirely new approach to building applications.


## In a Nutshell
Define your "capsules" (en-_capsulated_ pieces of state) at the top level:

```rust
// Capsules are simply functions that consume a CapsuleHandle.
// The CapsuleHandle lets you get the state of other capsules,
// in addition to using a large variety of side effects.

// This capsule provides the count and a way to increment that count.
fn count_manager(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn()) {
    let (count, set_count) = register(side_effects::state(0));
    let increment_count = || set_count(count + 1);
    (*count, increment_count)
}

// This capsule provides the count, plus one.
fn count_plus_one_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
    let (count, _increment_count) = get(count_manager);
    count + 1
}

let container = Container::new();
let ((count, increment_count), count_plus_one) = container.read((count, count_plus_one));
```

## Getting Started
Simply run:
`cargo add rearch`

Then, create one container for your application:
```dart
fn main() {
  let container = Container::new();
  // Use the container.
}
```

And take a look at the [examples] to get an idea on how to make some of your own capsules!

### Documentation
Also, there is some WIP [documentation] that will help you learn the core concepts behind ReArch!


## Minimum Supported Rust Version (MSRV)
The MSRV is currently 1.76.0 due to ReArch's dependency on
[`trait_upcasting`](https://github.com/rust-lang/rust/issues/65991).

The MSRV will only increment on minor or major changes (minor/major in terms of semver).

Further, it is worth mentioning that the example shown in "In a Nutshell" above requires nightly
for `unboxed_closures` and `fn_traits`, which is feature-gated under the `better-api` feature.
Once `unboxed_closures` and `fn_traits` stabilize,
this nightly syntax will be the preferred syntax,
and this will no longer be feature-gated.
(Without nightly, you must instead call the slightly more verbose
`get.get(some_capsule)` and `register.register(effect())`.)


## Help Wanted!
As much as I have done with ReArch, it always seems like there is more to do.
One person can only do so much!

If you would like to sponsor me to continue my work, thank you!
Sponsorship information should be up soon.

Or, if you would like to contribute, here are some areas where I would really appreciate help!
- Documentation (especially inline!)
  - If you could add code samples/improve clarity, that would be greatly appreciated.
- New side effects!
  - I've made many as I've needed them, but it'd be great to have more.
  - If you find yourself using a custom side effect over and over, consider making a PR!
Chances are other developers can use it too.

[documentation]: https://rearch.gsconrad.com
[examples]: https://github.com/GregoryConrad/rearch-rs/tree/main/examples
