use rearch::{CData, CapsuleHandle, Container};
use rearch_effects as effects;

fn count_capsule(_: CapsuleHandle) -> i32 {
    0
}

fn count_plus_one_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> i32 {
    get.get(count_capsule) + 1
}

fn crazy_capsule(_: CapsuleHandle) -> &'static str {
    "crazy!"
}

fn big_string_factory(
    CapsuleHandle { mut get, .. }: CapsuleHandle,
) -> impl CData + Fn(&str) -> String {
    let count = *get.get(count_capsule);
    let count_plus_one = *get.get(count_plus_one_capsule);
    let crazy = *get.get(crazy_capsule);
    move |other| {
        format!("param: {other}, count: {count}, count_plus_one: {count_plus_one}, crazy: {crazy}")
    }
}

fn uses_factory_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
    get.get(big_string_factory)("argument supplied to factory")
}

fn stateful_capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> (u32, impl CData + Fn(u32)) {
    let (state, set_state) = register.register(effects::state(0));
    (*state, set_state)
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
