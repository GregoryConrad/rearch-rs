use rearch::{side_effects, CapsuleHandle, Container};

fn count(_: CapsuleHandle) -> i32 {
    0
}

fn count_plus_one(CapsuleHandle { mut get, .. }: CapsuleHandle) -> i32 {
    get.get(count) + 1
}

fn crazy(CapsuleHandle { mut get, .. }: CapsuleHandle) -> &'static str {
    get.get(count);
    get.get(count_plus_one);
    "crazy!"
}

fn big_string_factory(
    CapsuleHandle { mut get, .. }: CapsuleHandle,
) -> impl Fn(&str) -> String + Clone + Send + Sync {
    let count = get.get(count);
    let count_plus_one = get.get(count_plus_one);
    let crazy = get.get(crazy);
    move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    }
}

fn uses_factory(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
    get.get(big_string_factory)("argument supplied to factory")
}

fn stateful(
    CapsuleHandle { register, .. }: CapsuleHandle,
) -> (u32, impl Fn(u32) + Clone + Send + Sync) {
    let (state, set_state) = register.register(side_effects::state(0));
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
