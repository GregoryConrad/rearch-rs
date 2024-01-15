use effects::Cloned;
use rearch::{CData, CapsuleHandle, Container};
use rearch_effects as effects;

fn count_capsule(_: CapsuleHandle) -> i32 {
    0
}

fn count_plus_one_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> i32 {
    get.as_ref(count_capsule) + 1
}

fn crazy_capsule(_: CapsuleHandle) -> &'static str {
    "crazy!"
}

fn big_string_factory(
    CapsuleHandle { mut get, .. }: CapsuleHandle,
) -> impl CData + Fn(&str) -> String {
    let count = *get.as_ref(count_capsule);
    let count_plus_one = *get.as_ref(count_plus_one_capsule);
    let crazy = *get.as_ref(crazy_capsule);
    move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    }
}

fn uses_factory_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
    get.as_ref(big_string_factory)("argument supplied to factory")
}

fn stateful_capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> (u32, impl CData + Fn(u32)) {
    register.register(effects::state::<Cloned<_>>(0))
}

fn main() {
    let container = Container::new();
    println!("{}", container.read(uses_factory_capsule));

    let (state, set_state) = container.read(stateful_capsule);
    println!("state = {state}");

    println!("Calling set_state(1)...");
    set_state(1);

    let (state, _) = container.read(stateful_capsule);
    println!("state = {state}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stateful_capsule_updates() {
        let container = Container::new();

        let (state, set_state) = container.read(stateful_capsule);
        assert_eq!(state, 0);

        set_state(1);

        let (state, _) = container.read(stateful_capsule);
        assert_eq!(state, 1);
    }

    #[test]
    fn factory_produces_correct_string() {
        assert_eq!(
            Container::new().read(big_string_factory)("arg"),
            "param: arg, count: 0, count_plus_one: 1, crazy: crazy!"
        );
    }
}
