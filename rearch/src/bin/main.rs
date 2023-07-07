use rearch::{capsule, side_effects, CapsuleReader, Container, SideEffectRegistrar};

#[capsule]
fn count() -> i32 {
    0
}

#[capsule]
fn count_plus_one() -> i32 {
    _count + 1
}

fn crazy(mut reader: CapsuleReader, _: SideEffectRegistrar) -> &'static str {
    reader.read(count);
    reader.read(count_plus_one);
    "crazy!"
}

fn big_string_factory(
    mut reader: CapsuleReader,
    _: SideEffectRegistrar,
) -> impl Fn(&str) -> String + Clone + Send + Sync {
    let count = reader.read(count);
    let count_plus_one = reader.read(count_plus_one);
    let crazy = reader.read(crazy);
    move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    }
}

#[capsule]
fn uses_factory() -> String {
    _big_string_factory("argument supplied to factory")
}

#[capsule]
fn stateful(registrar: SideEffectRegistrar) -> (u32, impl Fn(u32) + Clone + Send + Sync) {
    let (state, set_state) = registrar.register(side_effects::state(0));
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
