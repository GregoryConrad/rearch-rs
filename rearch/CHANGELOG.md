

## v0.5.2 (2023-12-14)

### Chore

 - <csr-id-864dee68d91c2f9e1bc5c6f18d9704b586413938/> update version numbers

### New Features

 - <csr-id-47e69e886c37b935c46405bde0ea6140c6ff80d5/> allow any Hash+Eq to be used as a capsule key

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Update version numbers ([`864dee6`](https://github.com/GregoryConrad/rearch-rs/commit/864dee68d91c2f9e1bc5c6f18d9704b586413938))
    - Allow any Hash+Eq to be used as a capsule key ([`47e69e8`](https://github.com/GregoryConrad/rearch-rs/commit/47e69e886c37b935c46405bde0ea6140c6ff80d5))
</details>

## v0.5.1 (2023-12-13)

<csr-id-66e664a6ae0160beea2874e924f64e3a5aba2154/>
<csr-id-63a5c5e5042387f91fedf5a26e0d14a412fcc27e/>

### Chore

 - <csr-id-66e664a6ae0160beea2874e924f64e3a5aba2154/> update rearch version number

### Performance

 - <csr-id-1ea4c0b922533a7a90bdee1baa63d53117414282/> wrap container values in Arc for faster clones

### Test

 - <csr-id-63a5c5e5042387f91fedf5a26e0d14a412fcc27e/> add better-api test for nightly

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release rearch v0.5.1 ([`f06ef85`](https://github.com/GregoryConrad/rearch-rs/commit/f06ef85e6a83ab3c4261a821071c5c2508e0f937))
    - Update rearch version number ([`66e664a`](https://github.com/GregoryConrad/rearch-rs/commit/66e664a6ae0160beea2874e924f64e3a5aba2154))
    - Add better-api test for nightly ([`63a5c5e`](https://github.com/GregoryConrad/rearch-rs/commit/63a5c5e5042387f91fedf5a26e0d14a412fcc27e))
    - Wrap container values in Arc for faster clones ([`1ea4c0b`](https://github.com/GregoryConrad/rearch-rs/commit/1ea4c0b922533a7a90bdee1baa63d53117414282))
</details>

## v0.5.0 (2023-12-12)

<csr-id-d42b38de26bd5f585e3c69728e7f4a7c40723cf1/>
<csr-id-89fef501de6d90bf0a0779720237fdc3f2366fd4/>
<csr-id-50c1439b2fba9893152fbe7fc7c051c7146d2efb/>
<csr-id-bd8ff2cd5d4125bf8a41dfdb36a384c272b751b5/>
<csr-id-302bf1364305b776c9c06abe25838f51504482aa/>

### Chore

 - <csr-id-d42b38de26bd5f585e3c69728e7f4a7c40723cf1/> rearch version bump

### Documentation

 - <csr-id-89181fc1885778207f05cdfcb774a085b204a13b/> update/fix SideEffectRegistrar docs

### Test

 - <csr-id-89fef501de6d90bf0a0779720237fdc3f2366fd4/> rewrite container thread safe test to be more idiomatic

### Refactor (BREAKING)

 - <csr-id-50c1439b2fba9893152fbe7fc7c051c7146d2efb/> remove redundant CapsuleKey bytes() API
 - <csr-id-bd8ff2cd5d4125bf8a41dfdb36a384c272b751b5/> hide more API members to permit future modification
 - <csr-id-302bf1364305b776c9c06abe25838f51504482aa/> improve/stabilize the CapsuleReader API

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release.
 - 5 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release rearch v0.5.0 ([`cbe7641`](https://github.com/GregoryConrad/rearch-rs/commit/cbe76417ff38ea079894793e5c53c803cc583656))
    - Rearch version bump ([`d42b38d`](https://github.com/GregoryConrad/rearch-rs/commit/d42b38de26bd5f585e3c69728e7f4a7c40723cf1))
    - Rewrite container thread safe test to be more idiomatic ([`89fef50`](https://github.com/GregoryConrad/rearch-rs/commit/89fef501de6d90bf0a0779720237fdc3f2366fd4))
    - Remove redundant CapsuleKey bytes() API ([`50c1439`](https://github.com/GregoryConrad/rearch-rs/commit/50c1439b2fba9893152fbe7fc7c051c7146d2efb))
    - Hide more API members to permit future modification ([`bd8ff2c`](https://github.com/GregoryConrad/rearch-rs/commit/bd8ff2cd5d4125bf8a41dfdb36a384c272b751b5))
    - Update/fix SideEffectRegistrar docs ([`89181fc`](https://github.com/GregoryConrad/rearch-rs/commit/89181fc1885778207f05cdfcb774a085b204a13b))
    - Improve/stabilize the CapsuleReader API ([`302bf13`](https://github.com/GregoryConrad/rearch-rs/commit/302bf1364305b776c9c06abe25838f51504482aa))
</details>

## v0.4.2 (2023-12-06)

<csr-id-4cb3de4f45d014a6a1edd130cfcd50b227604b34/>

### Chore

 - <csr-id-4cb3de4f45d014a6a1edd130cfcd50b227604b34/> update version numbers

### New Features

 - <csr-id-1d336b50adfe7dc43962ac6188fa4b21342d025a/> add dynamic capsule support

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 6 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#10](https://github.com/GregoryConrad/rearch-rs/issues/10)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#10](https://github.com/GregoryConrad/rearch-rs/issues/10)**
    - Add dynamic capsule support ([`1d336b5`](https://github.com/GregoryConrad/rearch-rs/commit/1d336b50adfe7dc43962ac6188fa4b21342d025a))
 * **Uncategorized**
    - Release rearch v0.4.2 ([`966bedd`](https://github.com/GregoryConrad/rearch-rs/commit/966bedd4f47d383d7f835dfdde5d32f1508638c0))
    - Update version numbers ([`4cb3de4`](https://github.com/GregoryConrad/rearch-rs/commit/4cb3de4f45d014a6a1edd130cfcd50b227604b34))
</details>

## v0.4.1 (2023-11-30)

<csr-id-762eb7bab9fcace1144a77697719a06b290153ff/>

### New Features

 - <csr-id-1dfa2d4c9e4e520798ba2d62ac3d06cf54247674/> add optional eq rebuild optimization

### Bug Fixes

 - <csr-id-f1efbcfbef09b2c36bf39120afbc60c0400c92ce/> make the as_listener side effect register ()

### Reverted

 - <csr-id-4427dd0786b73cfb4b760d4ac1f5525171f335e4/> switch style back to rust stable

### Style

 - <csr-id-762eb7bab9fcace1144a77697719a06b290153ff/> fix code formatting

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release rearch v0.4.1 ([`cc0fccf`](https://github.com/GregoryConrad/rearch-rs/commit/cc0fccf5c1fcb901ec4f8accc323e9ecef665eae))
    - Switch style back to rust stable ([`4427dd0`](https://github.com/GregoryConrad/rearch-rs/commit/4427dd0786b73cfb4b760d4ac1f5525171f335e4))
    - Make the as_listener side effect register () ([`f1efbcf`](https://github.com/GregoryConrad/rearch-rs/commit/f1efbcfbef09b2c36bf39120afbc60c0400c92ce))
    - Fix code formatting ([`762eb7b`](https://github.com/GregoryConrad/rearch-rs/commit/762eb7bab9fcace1144a77697719a06b290153ff))
    - Add optional eq rebuild optimization ([`1dfa2d4`](https://github.com/GregoryConrad/rearch-rs/commit/1dfa2d4c9e4e520798ba2d62ac3d06cf54247674))
</details>

## v0.4.0 (2023-11-29)

<csr-id-918a68fc8436e2a708bcde9e80b6e2eb5df8d4be/>
<csr-id-b606de4f60542de4c1ed7ad22cbf19ff1e10905a/>
<csr-id-970cd0bd5c82784b26dde1b169c3205593be76a1/>
<csr-id-55f7b69a43d3c47cbcccfefe3e290a32d3e955bd/>
<csr-id-ddb0cac0383aed6263ce4db04f3b3c982c838718/>
<csr-id-91fc15e7cdd460226bac37650bb8648179c7438a/>
<csr-id-97afad4fca9a0bd4b25277da1826d3a32f2e05ab/>
<csr-id-7d6e2e823484aaee3348edcc70e3082c84950fcd/>
<csr-id-e23e89d7bcde38d211aeae3ba57a14ab74794a81/>
<csr-id-bd902097056a3970ce0c8314ead48ad7627d97e7/>
<csr-id-b3186e4208c4a03abd6c11ca17b38a1d3029fb62/>
<csr-id-9cb2d62cdcf5c6331bb9947081c02f3f7943a0f8/>
<csr-id-df406f1347039a1ce6d0ae0791b15a7bc7a6869b/>
<csr-id-56837907b57d9fbd14b5ce839327e35de6b9b39f/>
<csr-id-c0012208413451a922faa38244555ece9db1763e/>
<csr-id-16d03972355ff974b702e51b6aac279d40587f85/>
<csr-id-b216db9b8a1effb6fc30b4f38d0a76a97e863107/>
<csr-id-92c7ff70c23167405f27817acd19396dc112b612/>
<csr-id-dde160ee14fc78c4a9b47b8ef38ff5bf7c272203/>
<csr-id-86582feb500dd369d97f1ac1fc52b5ced9d06bcb/>
<csr-id-a0c98ef1104e747e2d40e546dfe9e164ba18b41b/>
<csr-id-a96514a07e6f346b8664f8811b727d329fd6c669/>
<csr-id-2f290f9915106bfb73e5fa14b58dba16ded4ab3b/>
<csr-id-63e800cb55a4c192c0dc4b0c513cd7fa50c2669e/>
<csr-id-8673866d7d1a45a94a0b93315983ea648d2195e7/>
<csr-id-b2b5e92c28507773f88155c9598486352d10b0ee/>
<csr-id-951375b2232e982ec2c50de72c72759458b98eb7/>
<csr-id-09d13cab87f0737b679787de8151e990c7c75df2/>

### Chore

 - <csr-id-918a68fc8436e2a708bcde9e80b6e2eb5df8d4be/> touch new changelogs
 - <csr-id-b606de4f60542de4c1ed7ad22cbf19ff1e10905a/> version bumps in prep for smart-release
 - <csr-id-970cd0bd5c82784b26dde1b169c3205593be76a1/> version separation and bump
 - <csr-id-55f7b69a43d3c47cbcccfefe3e290a32d3e955bd/> update TODOs

### Documentation

 - <csr-id-35a2b98434f6bf9214ded3b17f6d886c370e8f61/> add/improve code comments and panics
 - <csr-id-1ececbcd31e323775051fc4628341d84993269f1/> update old terminology to idempotence
 - <csr-id-669e9bec352d5ebcec33c82c80d33ef08ffec7ac/> update terminology for release
 - <csr-id-2d651304526c601213fffbe122364a681f8cca29/> add some inline documentation
 - <csr-id-6e3369bfb7fb3caf669dee2f97be0a2f5099eaa1/> add project info to cargo manifest

### New Features

 - <csr-id-7c7f2bed80fd4b60dd19559b196f9a2f0283aaa6/> add as_listener side effect
 - <csr-id-40014016aaf29ab0511144c7acf2f7d8ed74d66d/> add prototype tokio side effects
 - <csr-id-2ef76075c424f2705e4228342fd3f82a12080fda/> state reducer side effects
 - <csr-id-c71b995a7421d8c10168a56927797fbed7b47473/> add new run_on_change effect
 - <csr-id-53c2041fa512dda9f543cc64cabb7c062640b01e/> add temporary container listeners
 - <csr-id-d7d191d16b6aeae73ab8bcd7ca98b779f163927d/> capsule macro
 - <csr-id-0cc3605932cacdb9f67f9aad205e399ac4ee290a/> feature gate the nightly-only api
 - <csr-id-f94f1203e03389848dff04677a6e666a65a4fc86/> update some old side effects
 - <csr-id-51ab97ce3699cc5c68b3739625f01a844d5ebac8/> functional side effects!
 - <csr-id-03aaed9648f6f8d34b23ecc39fa072c730e32205/> add CapsuleReader mocking
 - <csr-id-10272d55bb08bf0645847b9de31fbfd5ea00fda0/> get() capsule reader api
 - <csr-id-a0ba3c62044f4438ed4c22445ab36a0703e22090/> add no-arg register()
 - <csr-id-870c2503f7cc7752dac4675af9116e028aa79eb3/> basic fn capsule implementation
 - <csr-id-207e25d457cf3638d77404af3a412a175e824208/> add new side effects (and a couple todos)
 - <csr-id-c6f820900fb67935b02e1ceca78e1b38e239e13b/> add fundamental side effects
 - <csr-id-80560dd24183bdbe512602db801e1d1236033f05/> add cargo logging feature
 - <csr-id-614d0b3e05cef9595bfa2ced563f87da22526b3b/> add mutation side effect
 - <csr-id-4ae74de18d671781db54dd9e22d7486ed409cd5c/> add reducer side effects
 - <csr-id-340bff64a17c43330f804d2e4cf6cda1f834c396/> add new side effects
 - <csr-id-13005c197468a0db3e67eccb141e789fb2df35be/> add new side effects: effect, memo, future
 - <csr-id-f8be486b2541238493ecb8652c550d2c6885b9bd/> add working mvp

### Bug Fixes

 - <csr-id-60d34ac6be0f3505fe754d741183d816dd473bc0/> fix CapsuleReader when better-api feature enabled
 - <csr-id-a1a035eac6a9addcce021468f2d80db0c62e2052/> rewrite garbage collection to use more idiomatic code
 - <csr-id-baf1b125f8656fee912f503823df84fad4757092/> modify topological sort to fix edge-case double builds
 - <csr-id-702c95b634226f8c4243cb459bfea79fa814debb/> change build order to topological sort
 - <csr-id-7933d79bd31b3ecc7d00f26313edd703c29815f4/> building dependents with handles no longer panics

### Other

 - <csr-id-ddb0cac0383aed6263ce4db04f3b3c982c838718/> add quick benchmark info

### Performance

 - <csr-id-084076b28aaf27ebef1cdf0efb8fbd10146ae405/> remove a lot of dynamic dispatch

### Refactor

 - <csr-id-91fc15e7cdd460226bac37650bb8648179c7438a/> split up files and fix TODOs
 - <csr-id-97afad4fca9a0bd4b25277da1826d3a32f2e05ab/> split into multiple files for maintainability
 - <csr-id-7d6e2e823484aaee3348edcc70e3082c84950fcd/> move gc work to new file
 - <csr-id-e23e89d7bcde38d211aeae3ba57a14ab74794a81/> add safer ownership practices
 - <csr-id-bd902097056a3970ce0c8314ead48ad7627d97e7/> give capsules' build an &self
 - <csr-id-b3186e4208c4a03abd6c11ca17b38a1d3029fb62/> prep for new features
 - <csr-id-9cb2d62cdcf5c6331bb9947081c02f3f7943a0f8/> lifetime improvements and crate restructuring
 - <csr-id-df406f1347039a1ce6d0ae0791b15a7bc7a6869b/> code clean ups and improvements
 - <csr-id-56837907b57d9fbd14b5ce839327e35de6b9b39f/> remove unsafe using new CapsuleType
 - <csr-id-c0012208413451a922faa38244555ece9db1763e/> require Clone rather than Arc wrapping
 - <csr-id-16d03972355ff974b702e51b6aac279d40587f85/> support mutations with rebuilds
 - <csr-id-b216db9b8a1effb6fc30b4f38d0a76a97e863107/> improve node ownership model
 - <csr-id-92c7ff70c23167405f27817acd19396dc112b612/> switch to nightly and api clean up

### Style

 - <csr-id-dde160ee14fc78c4a9b47b8ef38ff5bf7c272203/> update to use Rust 1.74 workspace lints
 - <csr-id-86582feb500dd369d97f1ac1fc52b5ced9d06bcb/> make clippy happy with must_use
 - <csr-id-a0c98ef1104e747e2d40e546dfe9e164ba18b41b/> clean up Container::new and a TODO
 - <csr-id-a96514a07e6f346b8664f8811b727d329fd6c669/> make buggy clippy happy :)
 - <csr-id-2f290f9915106bfb73e5fa14b58dba16ded4ab3b/> fix some clippy lints in tests
 - <csr-id-63e800cb55a4c192c0dc4b0c513cd7fa50c2669e/> enable a handful of new clippy lints
 - <csr-id-8673866d7d1a45a94a0b93315983ea648d2195e7/> enable more clippy lints
 - <csr-id-b2b5e92c28507773f88155c9598486352d10b0ee/> clean up some clippy-suggested code
 - <csr-id-951375b2232e982ec2c50de72c72759458b98eb7/> fix clippy warnings from ci

### Test

 - <csr-id-09d13cab87f0737b679787de8151e990c7c75df2/> add in depth graph update test

### New Features (BREAKING)

 - <csr-id-2fbad41b0430fad3217a767444e7a32f42c535c6/> set MSRV to 1.76.0
 - <csr-id-c4b36fb8d65d98fac0a986b182dc5fcf7a4ed5ff/> add CData, rearch-tokio
 - <csr-id-5fbc2b57ff9bb4639fa6e4edbdb34dfb8a06cd04/> add the CapsuleHandle
 - <csr-id-9ca0da52a7ef053bfca7ef85fb6cbf1d0216d521/> function capsules! 🎉
 - <csr-id-673de4dd889ab4d695b0d1db61e429326ae13db7/> idk anymore 😵‍💫😵‍💫
 - <csr-id-ee936422e8beec0fe45c1f4e1a04707d47949074/> new system for handling side effects

### Bug Fixes (BREAKING)

 - <csr-id-78d1fa0162f25c9ee3d52fb86240c830a7a35032/> temporarily remove listen method until design is finalized

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 68 commits contributed to the release over the course of 174 calendar days.
 - 67 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release rearch-macros v0.4.0, rearch v0.4.0, rearch-tokio v0.4.0 ([`f1f5fe8`](https://github.com/GregoryConrad/rearch-rs/commit/f1f5fe8d9d5d66f8621bfbc599245a49b8767c04))
    - Touch new changelogs ([`918a68f`](https://github.com/GregoryConrad/rearch-rs/commit/918a68fc8436e2a708bcde9e80b6e2eb5df8d4be))
    - Version bumps in prep for smart-release ([`b606de4`](https://github.com/GregoryConrad/rearch-rs/commit/b606de4f60542de4c1ed7ad22cbf19ff1e10905a))
    - Version separation and bump ([`970cd0b`](https://github.com/GregoryConrad/rearch-rs/commit/970cd0bd5c82784b26dde1b169c3205593be76a1))
    - Set MSRV to 1.76.0 ([`2fbad41`](https://github.com/GregoryConrad/rearch-rs/commit/2fbad41b0430fad3217a767444e7a32f42c535c6))
    - Update to use Rust 1.74 workspace lints ([`dde160e`](https://github.com/GregoryConrad/rearch-rs/commit/dde160ee14fc78c4a9b47b8ef38ff5bf7c272203))
    - Add CData, rearch-tokio ([`c4b36fb`](https://github.com/GregoryConrad/rearch-rs/commit/c4b36fb8d65d98fac0a986b182dc5fcf7a4ed5ff))
    - Add/improve code comments and panics ([`35a2b98`](https://github.com/GregoryConrad/rearch-rs/commit/35a2b98434f6bf9214ded3b17f6d886c370e8f61))
    - Add as_listener side effect ([`7c7f2be`](https://github.com/GregoryConrad/rearch-rs/commit/7c7f2bed80fd4b60dd19559b196f9a2f0283aaa6))
    - Update old terminology to idempotence ([`1ececbc`](https://github.com/GregoryConrad/rearch-rs/commit/1ececbcd31e323775051fc4628341d84993269f1))
    - Temporarily remove listen method until design is finalized ([`78d1fa0`](https://github.com/GregoryConrad/rearch-rs/commit/78d1fa0162f25c9ee3d52fb86240c830a7a35032))
    - Fix CapsuleReader when better-api feature enabled ([`60d34ac`](https://github.com/GregoryConrad/rearch-rs/commit/60d34ac6be0f3505fe754d741183d816dd473bc0))
    - Update terminology for release ([`669e9be`](https://github.com/GregoryConrad/rearch-rs/commit/669e9bec352d5ebcec33c82c80d33ef08ffec7ac))
    - Add the CapsuleHandle ([`5fbc2b5`](https://github.com/GregoryConrad/rearch-rs/commit/5fbc2b57ff9bb4639fa6e4edbdb34dfb8a06cd04))
    - Add prototype tokio side effects ([`4001401`](https://github.com/GregoryConrad/rearch-rs/commit/40014016aaf29ab0511144c7acf2f7d8ed74d66d))
    - State reducer side effects ([`2ef7607`](https://github.com/GregoryConrad/rearch-rs/commit/2ef76075c424f2705e4228342fd3f82a12080fda))
    - Add new run_on_change effect ([`c71b995`](https://github.com/GregoryConrad/rearch-rs/commit/c71b995a7421d8c10168a56927797fbed7b47473))
    - Add temporary container listeners ([`53c2041`](https://github.com/GregoryConrad/rearch-rs/commit/53c2041fa512dda9f543cc64cabb7c062640b01e))
    - Capsule macro ([`d7d191d`](https://github.com/GregoryConrad/rearch-rs/commit/d7d191d16b6aeae73ab8bcd7ca98b779f163927d))
    - Feature gate the nightly-only api ([`0cc3605`](https://github.com/GregoryConrad/rearch-rs/commit/0cc3605932cacdb9f67f9aad205e399ac4ee290a))
    - Update some old side effects ([`f94f120`](https://github.com/GregoryConrad/rearch-rs/commit/f94f1203e03389848dff04677a6e666a65a4fc86))
    - Functional side effects! ([`51ab97c`](https://github.com/GregoryConrad/rearch-rs/commit/51ab97ce3699cc5c68b3739625f01a844d5ebac8))
    - Make clippy happy with must_use ([`86582fe`](https://github.com/GregoryConrad/rearch-rs/commit/86582feb500dd369d97f1ac1fc52b5ced9d06bcb))
    - Clean up Container::new and a TODO ([`a0c98ef`](https://github.com/GregoryConrad/rearch-rs/commit/a0c98ef1104e747e2d40e546dfe9e164ba18b41b))
    - Add CapsuleReader mocking ([`03aaed9`](https://github.com/GregoryConrad/rearch-rs/commit/03aaed9648f6f8d34b23ecc39fa072c730e32205))
    - Update TODOs ([`55f7b69`](https://github.com/GregoryConrad/rearch-rs/commit/55f7b69a43d3c47cbcccfefe3e290a32d3e955bd))
    - Split up files and fix TODOs ([`91fc15e`](https://github.com/GregoryConrad/rearch-rs/commit/91fc15e7cdd460226bac37650bb8648179c7438a))
    - Split into multiple files for maintainability ([`97afad4`](https://github.com/GregoryConrad/rearch-rs/commit/97afad4fca9a0bd4b25277da1826d3a32f2e05ab))
    - Move gc work to new file ([`7d6e2e8`](https://github.com/GregoryConrad/rearch-rs/commit/7d6e2e823484aaee3348edcc70e3082c84950fcd))
    - Get() capsule reader api ([`10272d5`](https://github.com/GregoryConrad/rearch-rs/commit/10272d55bb08bf0645847b9de31fbfd5ea00fda0))
    - Add no-arg register() ([`a0ba3c6`](https://github.com/GregoryConrad/rearch-rs/commit/a0ba3c62044f4438ed4c22445ab36a0703e22090))
    - Add safer ownership practices ([`e23e89d`](https://github.com/GregoryConrad/rearch-rs/commit/e23e89d7bcde38d211aeae3ba57a14ab74794a81))
    - Add quick benchmark info ([`ddb0cac`](https://github.com/GregoryConrad/rearch-rs/commit/ddb0cac0383aed6263ce4db04f3b3c982c838718))
    - Make buggy clippy happy :) ([`a96514a`](https://github.com/GregoryConrad/rearch-rs/commit/a96514a07e6f346b8664f8811b727d329fd6c669))
    - Function capsules! 🎉 ([`9ca0da5`](https://github.com/GregoryConrad/rearch-rs/commit/9ca0da52a7ef053bfca7ef85fb6cbf1d0216d521))
    - Basic fn capsule implementation ([`870c250`](https://github.com/GregoryConrad/rearch-rs/commit/870c2503f7cc7752dac4675af9116e028aa79eb3))
    - Give capsules' build an &self ([`bd90209`](https://github.com/GregoryConrad/rearch-rs/commit/bd902097056a3970ce0c8314ead48ad7627d97e7))
    - Prep for new features ([`b3186e4`](https://github.com/GregoryConrad/rearch-rs/commit/b3186e4208c4a03abd6c11ca17b38a1d3029fb62))
    - Idk anymore 😵‍💫😵‍💫 ([`673de4d`](https://github.com/GregoryConrad/rearch-rs/commit/673de4dd889ab4d695b0d1db61e429326ae13db7))
    - Lifetime improvements and crate restructuring ([`9cb2d62`](https://github.com/GregoryConrad/rearch-rs/commit/9cb2d62cdcf5c6331bb9947081c02f3f7943a0f8))
    - Fix some clippy lints in tests ([`2f290f9`](https://github.com/GregoryConrad/rearch-rs/commit/2f290f9915106bfb73e5fa14b58dba16ded4ab3b))
    - Enable a handful of new clippy lints ([`63e800c`](https://github.com/GregoryConrad/rearch-rs/commit/63e800cb55a4c192c0dc4b0c513cd7fa50c2669e))
    - Enable more clippy lints ([`8673866`](https://github.com/GregoryConrad/rearch-rs/commit/8673866d7d1a45a94a0b93315983ea648d2195e7))
    - Add new side effects (and a couple todos) ([`207e25d`](https://github.com/GregoryConrad/rearch-rs/commit/207e25d457cf3638d77404af3a412a175e824208))
    - Add fundamental side effects ([`c6f8209`](https://github.com/GregoryConrad/rearch-rs/commit/c6f820900fb67935b02e1ceca78e1b38e239e13b))
    - Code clean ups and improvements ([`df406f1`](https://github.com/GregoryConrad/rearch-rs/commit/df406f1347039a1ce6d0ae0791b15a7bc7a6869b))
    - Add cargo logging feature ([`80560dd`](https://github.com/GregoryConrad/rearch-rs/commit/80560dd24183bdbe512602db801e1d1236033f05))
    - New system for handling side effects ([`ee93642`](https://github.com/GregoryConrad/rearch-rs/commit/ee936422e8beec0fe45c1f4e1a04707d47949074))
    - Add some inline documentation ([`2d65130`](https://github.com/GregoryConrad/rearch-rs/commit/2d651304526c601213fffbe122364a681f8cca29))
    - Add project info to cargo manifest ([`6e3369b`](https://github.com/GregoryConrad/rearch-rs/commit/6e3369bfb7fb3caf669dee2f97be0a2f5099eaa1))
    - Remove unsafe using new CapsuleType ([`5683790`](https://github.com/GregoryConrad/rearch-rs/commit/56837907b57d9fbd14b5ce839327e35de6b9b39f))
    - Require Clone rather than Arc wrapping ([`c001220`](https://github.com/GregoryConrad/rearch-rs/commit/c0012208413451a922faa38244555ece9db1763e))
    - Rewrite garbage collection to use more idiomatic code ([`a1a035e`](https://github.com/GregoryConrad/rearch-rs/commit/a1a035eac6a9addcce021468f2d80db0c62e2052))
    - Modify topological sort to fix edge-case double builds ([`baf1b12`](https://github.com/GregoryConrad/rearch-rs/commit/baf1b125f8656fee912f503823df84fad4757092))
    - Add in depth graph update test ([`09d13ca`](https://github.com/GregoryConrad/rearch-rs/commit/09d13cab87f0737b679787de8151e990c7c75df2))
    - Add mutation side effect ([`614d0b3`](https://github.com/GregoryConrad/rearch-rs/commit/614d0b3e05cef9595bfa2ced563f87da22526b3b))
    - Change build order to topological sort ([`702c95b`](https://github.com/GregoryConrad/rearch-rs/commit/702c95b634226f8c4243cb459bfea79fa814debb))
    - Add reducer side effects ([`4ae74de`](https://github.com/GregoryConrad/rearch-rs/commit/4ae74de18d671781db54dd9e22d7486ed409cd5c))
    - Support mutations with rebuilds ([`16d0397`](https://github.com/GregoryConrad/rearch-rs/commit/16d03972355ff974b702e51b6aac279d40587f85))
    - Clean up some clippy-suggested code ([`b2b5e92`](https://github.com/GregoryConrad/rearch-rs/commit/b2b5e92c28507773f88155c9598486352d10b0ee))
    - Improve node ownership model ([`b216db9`](https://github.com/GregoryConrad/rearch-rs/commit/b216db9b8a1effb6fc30b4f38d0a76a97e863107))
    - Building dependents with handles no longer panics ([`7933d79`](https://github.com/GregoryConrad/rearch-rs/commit/7933d79bd31b3ecc7d00f26313edd703c29815f4))
    - Remove a lot of dynamic dispatch ([`084076b`](https://github.com/GregoryConrad/rearch-rs/commit/084076b28aaf27ebef1cdf0efb8fbd10146ae405))
    - Add new side effects ([`340bff6`](https://github.com/GregoryConrad/rearch-rs/commit/340bff64a17c43330f804d2e4cf6cda1f834c396))
    - Switch to nightly and api clean up ([`92c7ff7`](https://github.com/GregoryConrad/rearch-rs/commit/92c7ff70c23167405f27817acd19396dc112b612))
    - Fix clippy warnings from ci ([`951375b`](https://github.com/GregoryConrad/rearch-rs/commit/951375b2232e982ec2c50de72c72759458b98eb7))
    - Add new side effects: effect, memo, future ([`13005c1`](https://github.com/GregoryConrad/rearch-rs/commit/13005c197468a0db3e67eccb141e789fb2df35be))
    - Add working mvp ([`f8be486`](https://github.com/GregoryConrad/rearch-rs/commit/f8be486b2541238493ecb8652c550d2c6885b9bd))
</details>

