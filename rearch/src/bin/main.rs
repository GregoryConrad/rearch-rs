use rearch::{capsule, factory, BuiltinSideEffects, Container, SideEffectHandle};

#[capsule]
fn count() -> i32 {
    0
}

#[capsule]
fn count_plus_one(CountCapsule(count): CountCapsule) -> i32 {
    count + 1
}

#[capsule]
fn crazy(
    CountCapsule(_): CountCapsule,
    CountPlusOneCapsule(_): CountPlusOneCapsule,
) -> &'static str {
    "crazy!"
}

#[factory]
fn big_string(
    CountCapsule(count): CountCapsule,
    CountPlusOneCapsule(count_plus_one): CountPlusOneCapsule,
    CrazyCapsule(crazy): CrazyCapsule,
    (other,): (&str,),
) -> String {
    format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
}

#[capsule]
fn uses_factory(BigStringFactory(factory): BigStringFactory) -> String {
    factory("argument supplied to factory")
}

#[capsule]
fn stateful(handle: &mut impl SideEffectHandle) -> (u8, std::sync::Arc<dyn Fn(u8) + Sync + Send>) {
    let (state, set_state) = handle.state(0u8);
    (*state, std::sync::Arc::new(set_state))
}

fn main() {
    let container = Container::new();

    let uses_factory = rearch::read!(container, UsesFactoryCapsule);
    println!("{uses_factory}");

    let (state, set_state) = rearch::read!(container, StatefulCapsule);
    println!("{state}");
    set_state(1);

    let (state, _) = rearch::read!(container, StatefulCapsule);
    println!("{state}");
}
