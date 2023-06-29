use std::sync::Arc;

use rearch::{capsule, side_effects::StateEffect, CapsuleReader, Container, SideEffectRegistrar};

#[capsule]
fn count() -> i32 {
    0
}

#[capsule]
fn count_plus_one(reader: &mut impl CapsuleReader) -> i32 {
    reader.read(CountCapsule) + 1
}

#[capsule]
fn crazy(reader: &mut impl CapsuleReader) -> &'static str {
    reader.read(CountCapsule);
    reader.read(CountPlusOneCapsule);
    "crazy!"
}

#[capsule]
fn big_string_factory(
    reader: &mut impl CapsuleReader,
) -> Arc<dyn Fn(&str) -> String + Send + Sync> {
    let count = reader.read(CountCapsule);
    let count_plus_one = reader.read(CountPlusOneCapsule);
    let crazy = reader.read(CrazyCapsule);
    Arc::new(move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    })
}

#[capsule]
fn uses_factory(reader: &mut impl CapsuleReader) -> String {
    reader.read(BigStringFactoryCapsule)("argument supplied to factory")
}

#[capsule]
fn stateful(register: SideEffectRegistrar<'_>) -> (u8, std::sync::Arc<dyn Fn(u8) + Send + Sync>) {
    let (state, set_state) = register(StateEffect::new(0));
    (*state, set_state)
}

fn main() {
    let container = Container::new();

    let uses_factory = container.read(UsesFactoryCapsule);
    println!("{uses_factory}");

    let (state, set_state) = container.read(StatefulCapsule);
    assert_eq!(state, 0);

    set_state(1);
    let (state, _) = container.read(StatefulCapsule);
    assert_eq!(state, 1);
}
