# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## 0.3.0 - 2023-11-23
#### Features
- set MSRV to 1.76.0 - (2fbad41) - Gregory Conrad
#### Miscellaneous Chores
- version separation and bump - (970cd0b) - Gregory Conrad
#### Style
- update to use Rust 1.74 workspace lints - (dde160e) - Gregory Conrad

- - -

## 0.2.1 - 2023-11-06
#### Bug Fixes
- fix versioning issues - (e9e7041) - Gregory Conrad

- - -

## 0.2.0 - 2023-11-06
#### Documentation
- add examples and reword README - (0b8029f) - Gregory Conrad
- clarify code example in README - (ca6cd8a) - Gregory Conrad
- add/improve code comments and panics - (35a2b98) - Gregory Conrad
#### Features
- add CData, rearch-tokio - (c4b36fb) - Gregory Conrad
#### Style
- change variable names and improve rearch-axum example - (d0d33c4) - Gregory Conrad

- - -

## 0.1.0 - 2023-10-14
#### Bug Fixes
- temporarily remove listen method until design is finalized - (78d1fa0) - Gregory Conrad
- fix CapsuleReader when better-api feature enabled - (60d34ac) - Gregory Conrad
#### Continuous Integration
- add github release workflow - (d169bbc) - Gregory Conrad
#### Documentation
- add axum example - (55d39fc) - Gregory Conrad
- update old terminology to idempotence - (1ececbc) - Gregory Conrad
- add inline comments to README example - (11fcbb0) - Gregory Conrad
- finish README - (078222d) - Gregory Conrad
#### Features
- add as_listener side effect - (7c7f2be) - Gregory Conrad

- - -

## 0.0.1 - 2023-07-29
#### Bug Fixes
- rewrite garbage collection to use more idiomatic code - (a1a035e) - Gregory Conrad
- modify topological sort to fix edge-case double builds - (baf1b12) - Gregory Conrad
- change build order to topological sort - (702c95b) - Gregory Conrad
- building dependents with handles no longer panics - (7933d79) - Gregory Conrad
#### Continuous Integration
- switch toolchain to matrix in prep for stable support - (2a5250c) - Gregory Conrad
- harden the rust checks - (5bc8f35) - Gregory Conrad
- fix wasi to use nightly - (2a6b7ae) - Gregory Conrad
- add nightly toolchain to fix wasi tests - (c5a332e) - Gregory Conrad
- add dependabot and test workflow - (ef53c53) - Gregory Conrad
#### Documentation
- update terminology for release - (669e9be) - Gregory Conrad
- add functional bullet point to README - (880bf29) - Gregory Conrad
- fix broken image link - (3305b32) - Gregory Conrad
- add basic README header - (1024f17) - Gregory Conrad
- fix code examples in README - (88a876e) - Gregory Conrad
- add basic example to README - (29134f8) - Gregory Conrad
- add some inline documentation - (2d65130) - Gregory Conrad
- add project info to cargo manifest - (6e3369b) - Gregory Conrad
#### Features
- add the CapsuleHandle - (5fbc2b5) - Gregory Conrad
- add prototype tokio side effects - (4001401) - Gregory Conrad
- state reducer side effects - (2ef7607) - Gregory Conrad
- add new run_on_change effect - (c71b995) - Gregory Conrad
- add temporary container listeners - (53c2041) - Gregory Conrad
- capsule macro - (d7d191d) - Gregory Conrad
- feature gate the nightly-only api - (0cc3605) - Gregory Conrad
- update some old side effects - (f94f120) - Gregory Conrad
- functional side effects! - (51ab97c) - Gregory Conrad
- add CapsuleReader mocking - (03aaed9) - Gregory Conrad
- get() capsule reader api - (10272d5) - Gregory Conrad
- add no-arg register() - (a0ba3c6) - Gregory Conrad
- function capsules! 🎉 - (9ca0da5) - Gregory Conrad
- basic fn capsule implementation - (870c250) - Gregory Conrad
- idk anymore 😵‍💫😵‍💫 - (673de4d) - Gregory Conrad
- add new side effects (and a couple todos) - (207e25d) - Gregory Conrad
- add fundamental side effects - (c6f8209) - Gregory Conrad
- add cargo logging feature - (80560dd) - Gregory Conrad
- new system for handling side effects - (ee93642) - Gregory Conrad
- add mutation side effect - (614d0b3) - Gregory Conrad
- add reducer side effects - (4ae74de) - Gregory Conrad
- add new side effects - (340bff6) - Gregory Conrad
- add new side effects: effect, memo, future - (13005c1) - Gregory Conrad
#### Miscellaneous Chores
- update TODOs - (55f7b69) - Gregory Conrad
#### Performance Improvements
- remove a lot of dynamic dispatch - (084076b) - Gregory Conrad
#### Refactoring
- split up files and fix TODOs - (91fc15e) - Gregory Conrad
- split into multiple files for maintainability - (97afad4) - Gregory Conrad
- move gc work to new file - (7d6e2e8) - Gregory Conrad
- add safer ownership practices - (e23e89d) - Gregory Conrad
- give capsules' build an &self - (bd90209) - Gregory Conrad
- prep for new features - (b3186e4) - Gregory Conrad
- lifetime improvements and crate restructuring - (9cb2d62) - Gregory Conrad
- code clean ups and improvements - (df406f1) - Gregory Conrad
- remove unsafe using new CapsuleType - (5683790) - Gregory Conrad
- require Clone rather than Arc wrapping - (c001220) - Gregory Conrad
- support mutations with rebuilds - (16d0397) - Gregory Conrad
- improve node ownership model - (b216db9) - Gregory Conrad
- switch to nightly and api clean up - (92c7ff7) - Gregory Conrad
#### Style
- make clippy happy with must_use - (86582fe) - Gregory Conrad
- clean up Container::new and a TODO - (a0c98ef) - Gregory Conrad
- make buggy clippy happy :) - (a96514a) - Gregory Conrad
- fix some clippy lints in tests - (2f290f9) - Gregory Conrad
- enable a handful of new clippy lints - (63e800c) - Gregory Conrad
- enable more clippy lints - (8673866) - Gregory Conrad
- clean up some clippy-suggested code - (b2b5e92) - Gregory Conrad
- fix clippy warnings from ci - (951375b) - Gregory Conrad
#### Tests
- add in depth graph update test - (09d13ca) - Gregory Conrad

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).