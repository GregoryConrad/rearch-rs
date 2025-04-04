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

> We must state definitions and provide for priorities and descriptions of data.
> We must state relationships, not procedures.
>
> -- Grace Murray Hopper, _Management and the Computer of the Future_ (1962)
</p>

---


## Features
Specifically, ReArch is a novel solution to:
- ⚡️ State Management
- 🧮 Incremental Computation
- 🧱 Component-Based Software Engineering

And with those, come:
- Reactivity through declarative code
- Loose coupling and high testability
- App-level composability via a functional approach to dependency inversion
- [Feature composition through _side effects_](https://blog.gsconrad.com/2023/12/22/the-problem-with-state-management.html)


## In a Nutshell
Define your "capsules" (en-_capsulated_ pieces of state) at the top level:

```rust
// Capsules are simply functions that consume a CapsuleHandle.
// The CapsuleHandle lets you get the state of other capsules,
// in addition to using a large variety of side effects.

// This capsule provides the count and a way to increment that count.
fn count_manager(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn()) {
    let (count, set_count) = register(effects::state::<Cloned<_>>(0));
    let increment_count = move || set_count(count + 1);
    (count, increment_count)
}

// This capsule provides the count, plus one.
fn count_plus_one_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
    let (count, _increment_count) = get(count_manager);
    count + 1
}

let container = Container::new();

let ((count, increment_count), count_plus_one) =
    container.read((count_manager, count_plus_one_capsule));
assert_eq!(count, 0);
assert_eq!(count_plus_one, 1);

increment_count();

let ((count, _), count_plus_one) =
    container.read((count_manager, count_plus_one_capsule));
assert_eq!(count, 1);
assert_eq!(count_plus_one, 2);
```

> [!NOTE]
> The syntax used in the example above requires nightly for `unboxed_closures` and `fn_traits`,
> which is feature-gated under the `experimental-api` feature.
> Once `unboxed_closures` and `fn_traits` stabilize,
> this nightly syntax will be the preferred syntax,
> and this will no longer be feature-gated.
> (Without nightly, you must instead call the slightly more verbose
> `get.as_ref(some_capsule).clone()` and `register.register(effect())`.)


## Getting Started
Simply run:
`cargo add rearch rearch-effects`

Then, create one container for your application:
```rust
use rearch::*;
use rearch_effects as effects;

fn main() {
  let container = Container::new();
  // Use the container.
}
```

And take a look at the [examples] to get an idea on how to make some of your own capsules!

### Documentation
Also, there is some WIP [documentation] that will help you learn the core concepts behind ReArch!


## Minimum Supported Rust Version (MSRV)
The MSRV is the current stable Rust version for all crates in this repo.
As such, any new release may require an updated Rust installation.


## Help Wanted!
As much as I have done with ReArch, it always seems like there is more to do.
One person can only do so much!

If you would like to contribute, here are some areas where I would really appreciate help!
- Documentation (especially inline!)
  - If you could add code samples/improve clarity, that would be greatly appreciated.
- New side effects!
  - I've made many as I've needed them, but it'd be great to have more.
  - If you find yourself using a custom side effect over and over, consider making a PR!
Chances are other developers can use it too.

## Sponsors
You can become a sponsor of my work [here!](https://github.com/sponsors/GregoryConrad)
<p align="center">
  <img src="https://raw.githubusercontent.com/GregoryConrad/GregoryConrad/main/sponsorkit/sponsors.svg"/>
</p>

[documentation]: https://rearch.gsconrad.com
[examples]: https://github.com/GregoryConrad/rearch-rs/tree/main/examples
