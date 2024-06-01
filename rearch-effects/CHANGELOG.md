# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.5.2 (2024-06-01)

### Performance

 - <csr-id-2e072440d6b7ac7baaf2731576e5427387daa6ae/> switch multi side effect to Cell from RefCell

### Refactor

 - <csr-id-52f65a8b0cdb04e79a372a252cb48d21258ec670/> list out deps inline to be more idiomatic
 - <csr-id-0c264cfbbd49b155880aef45465eb54125511d1a/> remove once_cell dependency for lazy transformers

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 4 calendar days.
 - 31 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#55](https://github.com/GregoryConrad/rearch-rs/issues/55)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#55](https://github.com/GregoryConrad/rearch-rs/issues/55)**
    - List out deps inline to be more idiomatic ([`52f65a8`](https://github.com/GregoryConrad/rearch-rs/commit/52f65a8b0cdb04e79a372a252cb48d21258ec670))
 * **Uncategorized**
    - Remove once_cell dependency for lazy transformers ([`0c264cf`](https://github.com/GregoryConrad/rearch-rs/commit/0c264cfbbd49b155880aef45465eb54125511d1a))
    - Switch multi side effect to Cell from RefCell ([`2e07244`](https://github.com/GregoryConrad/rearch-rs/commit/2e072440d6b7ac7baaf2731576e5427387daa6ae))
</details>

## v0.5.1 (2024-04-30)

### New Features

 - <csr-id-657889b54dc35152f1674eaead3c88c9ca5f9f42/> add convenience MultiSideEffectRegistrar

### Performance

 - <csr-id-81184d5645146bb19d8653477bb6256969f8261d/> remove unnecessary clone

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 25 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#51](https://github.com/GregoryConrad/rearch-rs/issues/51)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#51](https://github.com/GregoryConrad/rearch-rs/issues/51)**
    - Add convenience MultiSideEffectRegistrar ([`657889b`](https://github.com/GregoryConrad/rearch-rs/commit/657889b54dc35152f1674eaead3c88c9ca5f9f42))
 * **Uncategorized**
    - Release rearch-effects v0.5.1 ([`153738f`](https://github.com/GregoryConrad/rearch-rs/commit/153738fd5554ba68b74f5b418487e3c2bc44fa05))
    - Remove unnecessary clone ([`81184d5`](https://github.com/GregoryConrad/rearch-rs/commit/81184d5645146bb19d8653477bb6256969f8261d))
</details>

## v0.5.0 (2024-04-05)

<csr-id-f9ab7a02192bf7555b26623081a85769d973a7ac/>
<csr-id-78eebcdd521a837d03427f52507691944155779c/>

### Chore

 - <csr-id-f9ab7a02192bf7555b26623081a85769d973a7ac/> bump version numbers

### Style

 - <csr-id-78eebcdd521a837d03427f52507691944155779c/> fix latest nightly clippy lint

### New Features (BREAKING)

 - <csr-id-0cca3369ce72c9ebbe5f5385dbe2e3e665fa2fd8/> add lifetimes to FnOnce callbacks
   Helps to reduce some otherwise unneeded clones.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 45 calendar days.
 - 80 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#40](https://github.com/GregoryConrad/rearch-rs/issues/40)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#40](https://github.com/GregoryConrad/rearch-rs/issues/40)**
    - Add lifetimes to FnOnce callbacks ([`0cca336`](https://github.com/GregoryConrad/rearch-rs/commit/0cca3369ce72c9ebbe5f5385dbe2e3e665fa2fd8))
 * **Uncategorized**
    - Release rearch v0.10.0, rearch-effects v0.5.0, rearch-tokio v0.10.0 ([`850e353`](https://github.com/GregoryConrad/rearch-rs/commit/850e353051de1d5970b34e8c7d75114f5f24db34))
    - Bump version numbers ([`f9ab7a0`](https://github.com/GregoryConrad/rearch-rs/commit/f9ab7a02192bf7555b26623081a85769d973a7ac))
    - Fix latest nightly clippy lint ([`78eebcd`](https://github.com/GregoryConrad/rearch-rs/commit/78eebcdd521a837d03427f52507691944155779c))
</details>

## v0.4.0 (2024-01-16)

<csr-id-88585638e2790125a3c47941b1b6dedf77209603/>

### Chore

 - <csr-id-88585638e2790125a3c47941b1b6dedf77209603/> update version numbers

### New Features (BREAKING)

 - <csr-id-0f8e8643df4a521e142c64f8eab1dad0b36d06d7/> add side effect state transformers

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 4 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#35](https://github.com/GregoryConrad/rearch-rs/issues/35)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#35](https://github.com/GregoryConrad/rearch-rs/issues/35)**
    - Add side effect state transformers ([`0f8e864`](https://github.com/GregoryConrad/rearch-rs/commit/0f8e8643df4a521e142c64f8eab1dad0b36d06d7))
 * **Uncategorized**
    - Release rearch v0.9.2, rearch-effects v0.4.0, rearch-tokio v0.9.0 ([`6fa2389`](https://github.com/GregoryConrad/rearch-rs/commit/6fa238941f6503c0a342e4ccc9ede7779b2c7d19))
    - Update version numbers ([`8858563`](https://github.com/GregoryConrad/rearch-rs/commit/88585638e2790125a3c47941b1b6dedf77209603))
</details>

## v0.3.0 (2024-01-11)

<csr-id-151ff0b918e0b43bb9c78c42d380aee29717409c/>

### Chore

 - <csr-id-151ff0b918e0b43bb9c78c42d380aee29717409c/> bump version numbers

### New Features (BREAKING)

 - <csr-id-8603fc98fad5d41684c3819b508dd67e844ffb63/> re-add and modernize older side effects

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 5 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#33](https://github.com/GregoryConrad/rearch-rs/issues/33)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#33](https://github.com/GregoryConrad/rearch-rs/issues/33)**
    - Re-add and modernize older side effects ([`8603fc9`](https://github.com/GregoryConrad/rearch-rs/commit/8603fc98fad5d41684c3819b508dd67e844ffb63))
 * **Uncategorized**
    - Release rearch v0.9.1, rearch-effects v0.3.0, rearch-tokio v0.8.0 ([`86c6afe`](https://github.com/GregoryConrad/rearch-rs/commit/86c6afe4f2958b611723e143a1928411b92a16f2))
    - Bump version numbers ([`151ff0b`](https://github.com/GregoryConrad/rearch-rs/commit/151ff0b918e0b43bb9c78c42d380aee29717409c))
</details>

## v0.2.1 (2024-01-06)

<csr-id-5ff6a4dcf9d0de3a5143f0c0ea584975558da99d/>

### Chore

 - <csr-id-5ff6a4dcf9d0de3a5143f0c0ea584975558da99d/> update version numbers

### Documentation

 - <csr-id-7783d3515ebf36bd007b5e77e41b6bf78ae10327/> update rearch-effects changelog

### New Features

 - <csr-id-c6f9a315e2bda23e5702508e4f6e1c1502de80e6/> cloneable side effects

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 4 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#23](https://github.com/GregoryConrad/rearch-rs/issues/23)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#23](https://github.com/GregoryConrad/rearch-rs/issues/23)**
    - Cloneable side effects ([`c6f9a31`](https://github.com/GregoryConrad/rearch-rs/commit/c6f9a315e2bda23e5702508e4f6e1c1502de80e6))
 * **Uncategorized**
    - Update rearch-effects changelog ([`7783d35`](https://github.com/GregoryConrad/rearch-rs/commit/7783d3515ebf36bd007b5e77e41b6bf78ae10327))
    - Update version numbers ([`5ff6a4d`](https://github.com/GregoryConrad/rearch-rs/commit/5ff6a4dcf9d0de3a5143f0c0ea584975558da99d))
</details>

## v0.2.0 (2024-01-01)

<csr-id-b81740f1787dd55c792b62dbf61295bcfbda6eba/>
<csr-id-ffda1036991653439cb71eb34bdae3cba710b065/>

### Chore

 - <csr-id-b81740f1787dd55c792b62dbf61295bcfbda6eba/> update version numbers

### Refactor (BREAKING)

 - <csr-id-ffda1036991653439cb71eb34bdae3cba710b065/> switch SideEffect to GAT lifetime

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 6 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#22](https://github.com/GregoryConrad/rearch-rs/issues/22)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#22](https://github.com/GregoryConrad/rearch-rs/issues/22)**
    - Switch SideEffect to GAT lifetime ([`ffda103`](https://github.com/GregoryConrad/rearch-rs/commit/ffda1036991653439cb71eb34bdae3cba710b065))
 * **Uncategorized**
    - Update version numbers ([`b81740f`](https://github.com/GregoryConrad/rearch-rs/commit/b81740f1787dd55c792b62dbf61295bcfbda6eba))
</details>

## v0.1.0 (2023-12-26)

<csr-id-d00c317c58da5bd9427333bb9527575d5049d62f/>

### Refactor (BREAKING)

 - <csr-id-d00c317c58da5bd9427333bb9527575d5049d62f/> move side effects to their own crate

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#20](https://github.com/GregoryConrad/rearch-rs/issues/20)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#20](https://github.com/GregoryConrad/rearch-rs/issues/20)**
    - Move side effects to their own crate ([`d00c317`](https://github.com/GregoryConrad/rearch-rs/commit/d00c317c58da5bd9427333bb9527575d5049d62f))
</details>

