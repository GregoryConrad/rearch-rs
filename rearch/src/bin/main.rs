use std::sync::Arc;

use rearch::{side_effects::StateEffect, CapsuleReader, Container, SideEffectRegistrar};

fn count(_: CapsuleReader, _: SideEffectRegistrar) -> i32 {
    0
}

fn count_plus_one(mut reader: CapsuleReader, _: SideEffectRegistrar) -> i32 {
    reader.read(count) + 1
}

fn crazy(mut reader: CapsuleReader, _: SideEffectRegistrar) -> &'static str {
    reader.read(count);
    reader.read(count_plus_one);
    "crazy!"
}

fn big_string_factory(
    mut reader: CapsuleReader,
    _: SideEffectRegistrar,
) -> Arc<dyn Fn(&str) -> String + Send + Sync> {
    let count = reader.read(count);
    let count_plus_one = reader.read(count_plus_one);
    let crazy = reader.read(crazy);
    Arc::new(move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    })
}

fn uses_factory(mut reader: CapsuleReader, _: SideEffectRegistrar) -> String {
    reader.read(big_string_factory)("argument supplied to factory")
}

fn stateful(
    _: CapsuleReader,
    register: SideEffectRegistrar,
) -> (u32, Arc<dyn Fn(u32) + Send + Sync>) {
    let (state, set_state) = register(StateEffect::new(0));
    (*state, set_state)
}

fn main() {
    let container = Container::new();
    println!("{}", container.read(uses_factory));

    let (state, set_state) = container.read(stateful);
    assert_eq!(state, 0);

    set_state(1);
    let (state, _) = container.read(stateful);
    assert_eq!(state, 1);

    // Quick little benchmark to test graph update speeds and get a flamegraph
    for i in 0..2_000_000 {
        set_state(i);
    }
}
