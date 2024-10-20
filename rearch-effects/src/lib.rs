use rearch::{CData, SideEffect, SideEffectRegistrar};
use std::sync::Arc;

mod state_transformers;
pub use state_transformers::*;

mod multi;
pub use multi::*;

mod overridable_capsule;
pub use overridable_capsule::{overridable_capsule, OverridableCapsule};

mod effect_lifetime_fixers;
use effect_lifetime_fixers::{EffectLifetimeFixer0, EffectLifetimeFixer1, EffectLifetimeFixer2};

/// A way to re-use the same exact side effect code while providing a different [`SideEffect::Api`]
/// based on the data you have and the data you want in return (such as a [`Clone`] or a ref).
///
/// See the implementors of this trait of the various state transformers you can use.
pub trait StateTransformer: Send + 'static {
    type Input;
    fn from_input(input: Self::Input) -> Self;

    type Inner;
    fn as_inner(&mut self) -> &mut Self::Inner;

    type Output<'a>;
    fn as_output(&mut self) -> Self::Output<'_>;
}

type SideEffectMutation<'f, ST> = Box<dyn 'f + FnOnce(&mut <ST as StateTransformer>::Inner)>;

/// A no-op side effect that specifies non-idempotence.
///
/// Useful so that a capsule can be treated as a listener/will not get
/// idempotent garbage collected from a container.
// NOTE: returns (), the no-op side effect
#[must_use]
pub fn as_listener() -> impl for<'a> SideEffect<Api<'a> = ()> {}

/// Analogous to [`SideEffectRegistrar::raw`], but uses a [`StateTransformer`] to specify the api.
#[allow(
    clippy::type_complexity,
    reason = "Return type refactor would require breaking change"
)]
pub fn raw<ST: StateTransformer>(
    initial: ST::Input,
) -> impl for<'a> SideEffect<
    Api<'a> = (
        ST::Output<'a>,
        impl CData + for<'f> Fn(Box<dyn 'f + FnOnce(&mut ST::Inner)>),
        Arc<dyn Send + Sync + for<'f> Fn(Box<dyn 'f + FnOnce()>)>,
    ),
> {
    EffectLifetimeFixer2::<_, ST>::new(move |register: SideEffectRegistrar| {
        let (transformer, run_mutation, run_txn) = register.raw(ST::from_input(initial));
        (
            transformer.as_output(),
            move |mutation: SideEffectMutation<ST>| {
                run_mutation(Box::new(move |st| mutation(st.as_inner())));
            },
            run_txn,
        )
    })
}

/// Similar to `useState` from React hooks.
/// Provides a copy of some state and a way to set that state via a callback.
pub fn state<ST: StateTransformer>(
    initial: ST::Input,
) -> impl for<'a> SideEffect<Api<'a> = (ST::Output<'a>, impl CData + Fn(ST::Inner))> {
    EffectLifetimeFixer1::<_, ST>::new(move |register: SideEffectRegistrar| {
        let (state, rebuild, _) = register.register(raw::<ST>(initial));
        let set_state = move |new_state| {
            rebuild(Box::new(|state| *state = new_state));
        };
        (state, set_state)
    })
}

/// Provides the same given value across builds.
pub fn value<ST: StateTransformer>(
    value: ST::Input,
) -> impl for<'a> SideEffect<Api<'a> = ST::Output<'a>> {
    EffectLifetimeFixer0::<_, ST>::new(move |register: SideEffectRegistrar| {
        register.register(raw::<ST>(value)).0
    })
}

/// Provides whether or not this is the first build being called.
#[must_use]
pub fn is_first_build() -> impl for<'a> SideEffect<Api<'a> = bool> {
    |register: SideEffectRegistrar| {
        let has_built_before = register.register(value::<MutRef<_>>(false));
        let is_first_build = !*has_built_before;
        *has_built_before = true;
        is_first_build
    }
}

/// Models the state reducer pattern via side effects (similar to `useReducer` from React hooks).
///
/// This should normally *not* be used with [`MutRef`].
pub fn reducer<ST: StateTransformer, Action, Reducer>(
    initial: ST::Input,
    reducer: Reducer,
) -> impl for<'a> SideEffect<Api<'a> = (ST::Output<'a>, impl CData + Fn(Action))>
where
    Action: 'static,
    Reducer: Clone + Send + Sync + 'static + Fn(&ST::Inner, Action) -> ST::Inner,
{
    EffectLifetimeFixer1::<_, ST>::new(move |register: SideEffectRegistrar| {
        let (state, update_state, _) = register.register(raw::<ST>(initial));
        (state, move |action| {
            update_state(Box::new(|state| *state = reducer(state, action)));
        })
    })
}

// NOTE: Commented out because I think people should really be using a hydrate equivalent
// instead of this. Probably value::<LazyMutRef<_>>() and run_on_change?
//
// /// A thin wrapper around the state side effect that enables easy state persistence.
// ///
// /// You provide a `read` function and a `write` function,
// /// and you receive the status of the latest read/write operation,
// /// in addition to a persist function that persists new state and triggers rebuilds.
// ///
// /// Note: when possible, it is highly recommended to use async persist instead of sync persist.
// /// This effect is blocking, which will prevent other capsule updates.
// /// However, this function is perfect for quick I/O, like when using something similar to redb.
// pub fn persist<Read, Write, R, T>(
//     read: Read,
//     write: Write,
// ) -> impl for<'a> SideEffect<Api<'a> = (&'a R, impl CData + Fn(T))>
// where
//     T: Send + 'static,
//     R: Send + 'static,
//     Read: Send + 'static + FnOnce() -> R,
//     Write: Clone + Send + Sync + 'static + Fn(T) -> R,
// {
//     EffectLifetimeFixer1::new(move |register: SideEffectRegistrar| {
//         let (state, set_state) = register.register(lazy_state(read));
//         let persist = move |new_data| set_state(write(new_data));
//         (&*state, persist)
//     })
// }

// NOTE: Commented out because this currently fails to compile due to the
// higher kinded lifetime bound on the nested opaque type (Api<'a> = impl Trait + 'a)
/*
/// Side effect that runs a callback whenever it changes and is dropped.
/// Similar to `useEffect` from React.
#[must_use]
pub fn run_on_change<F>() -> impl for<'a> SideEffect<Api<'a> = impl FnMut(F) + 'a>
where
    F: FnOnce() + Send + 'static,
{
    move |register: SideEffectRegistrar| {
        let state = register.register(value(FunctionalDrop(None)));
        // The old callback, if there is one, will be called when it is dropped,
        // via the `*state = ...` assignment below
        |callback| *state = FunctionalDrop(Some(callback))
    }
}
struct FunctionalDrop<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Drop for FunctionalDrop<F> {
    fn drop(&mut self) {
        if let Some(callback) = std::mem::take(&mut self.0) {
            callback();
        }
    }
}
#[must_use]
pub fn run_on_change2<F>() -> RunOnChange<F>
where
    F: FnOnce() + Send + 'static,
{
    RunOnChange(std::marker::PhantomData)
}
pub struct RunOnChange<F>(std::marker::PhantomData<F>);
impl<F: Send + FnOnce() + 'static> SideEffect for RunOnChange<F> {
    type Api<'registrar> = impl FnMut(F) + 'registrar;

    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        let state = registrar.register(value(FunctionalDrop(None)));
        // The old callback, if there is one, will be called when it is dropped,
        // via the `*state = ...` assignment below
        |callback| *state = FunctionalDrop(Some(callback))
    }
}
*/

#[cfg(test)]
mod tests {
    use crate::*;
    use rearch::{CapsuleHandle, Container};
    use std::sync::atomic::{AtomicU8, Ordering};

    // NOTE: raw side effect is effectively tested via combination of the other side effects

    #[allow(clippy::needless_pass_by_value)]
    fn assert_type<Expected>(_actual: Expected) {}

    #[test]
    fn transformer_output_types() {
        fn dummy_capsule(CapsuleHandle { register, .. }: CapsuleHandle) {
            let ((r, _, _), (mr, _, _), (c, _, _)) = register.register((
                raw::<Ref<u8>>(123),
                raw::<MutRef<u8>>(123),
                raw::<Cloned<u8>>(123),
            ));
            assert_type::<&u8>(r);
            assert_type::<&mut u8>(mr);
            assert_type::<u8>(c);
        }
        Container::new().read(dummy_capsule);
    }

    #[test]
    fn lazy_transformer_output_types() {
        fn dummy_capsule(CapsuleHandle { register, .. }: CapsuleHandle) {
            let ((r, _, _), (mr, _, _), (c, _, _)) = register.register((
                raw::<LazyRef<_>>(|| 123),
                raw::<LazyMutRef<_>>(|| 123),
                raw::<LazyCloned<_>>(|| 123),
            ));
            assert_type::<&u8>(r);
            assert_type::<&mut u8>(mr);
            assert_type::<u8>(c);
        }
        Container::new().read(dummy_capsule);
    }

    #[test]
    fn lazy_transformer_invokes_init_fn() {
        fn lazy_transformer_capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> u8 {
            register.register(value::<LazyCloned<_>>(|| 123))
        }
        assert_eq!(Container::new().read(lazy_transformer_capsule), 123);
    }

    #[test]
    fn as_listener_gets_changes() {
        static BUILD_COUNT: AtomicU8 = AtomicU8::new(0);

        fn rebuildable_capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> impl CData + Fn() {
            let ((), rebuild, _) = register.raw(());
            move || rebuild(Box::new(|()| {}))
        }

        fn listener_capsule(CapsuleHandle { mut get, register }: CapsuleHandle) {
            register.register(as_listener());
            BUILD_COUNT.fetch_add(1, Ordering::SeqCst);
            get.as_ref(rebuildable_capsule);
        }

        let container = Container::new();
        container.read(listener_capsule);
        container.read(rebuildable_capsule)();
        assert_eq!(BUILD_COUNT.fetch_add(1, Ordering::SeqCst), 2);
    }

    #[test]
    fn state_can_change() {
        fn stateful_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (u8, impl CData + Fn(u8)) {
            register.register(state::<Cloned<_>>(0))
        }

        let container = Container::new();
        assert_eq!(container.read(stateful_capsule).0, 0);
        container.read(stateful_capsule).1(1);
        assert_eq!(container.read(stateful_capsule).0, 1);
    }

    #[test]
    fn value_can_change() {
        fn rebuildable_capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> impl CData + Fn() {
            let ((), rebuild, _) = register.raw(());
            move || rebuild(Box::new(|()| {}))
        }

        fn build_count_capsule(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            get.as_ref(rebuildable_capsule);
            let build_count = register.register(value::<MutRef<_>>(0));
            *build_count += 1;
            *build_count
        }

        let container = Container::new();
        assert_eq!(container.read(build_count_capsule), 1);
        container.read(rebuildable_capsule)();
        assert_eq!(container.read(build_count_capsule), 2);
        container.read(rebuildable_capsule)();
        assert_eq!(container.read(build_count_capsule), 3);
    }

    #[test]
    fn is_first_build_changes_state() {
        fn is_first_build_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (bool, impl CData + Fn()) {
            let (is_first_build, ((), rebuild, _)) =
                register.register((is_first_build(), raw::<MutRef<_>>(())));
            (is_first_build, move || rebuild(Box::new(|()| {})))
        }

        let container = Container::new();
        assert!(container.read(is_first_build_capsule).0);
        container.read(is_first_build_capsule).1();
        assert!(!container.read(is_first_build_capsule).0);
        container.read(is_first_build_capsule).1();
        assert!(!container.read(is_first_build_capsule).0);
    }

    #[test]
    fn reducer_can_change() {
        enum CountAction {
            Increment,
            Decrement,
        }

        fn count_manager(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (u8, impl CData + Fn(CountAction)) {
            register.register(reducer::<Cloned<_>, _, _>(
                0,
                |state, action| match action {
                    CountAction::Increment => state + 1,
                    CountAction::Decrement => state - 1,
                },
            ))
        }

        let container = Container::new();
        assert_eq!(container.read(count_manager).0, 0);
        container.read(count_manager).1(CountAction::Increment);
        assert_eq!(container.read(count_manager).0, 1);
        container.read(count_manager).1(CountAction::Decrement);
        assert_eq!(container.read(count_manager).0, 0);
    }
}
