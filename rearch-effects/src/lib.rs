use once_cell::unsync::Lazy;
use rearch::{CData, SideEffect, SideEffectRegistrar};
use std::sync::Arc;

pub mod cloneable;

// This workaround was derived from:
// https://github.com/GregoryConrad/rearch-rs/issues/3#issuecomment-1872869363
// And is needed because of:
// https://github.com/rust-lang/rust/issues/111662
use effect_lifetime_fixers::{EffectLifetimeFixer0, EffectLifetimeFixer1, EffectLifetimeFixer2};
mod effect_lifetime_fixers {
    use rearch::{SideEffect, SideEffectRegistrar};

    pub struct EffectLifetimeFixer0<F>(F);
    impl<T, F> SideEffect for EffectLifetimeFixer0<F>
    where
        T: Send + 'static,
        F: FnOnce(SideEffectRegistrar) -> &mut T,
    {
        type Api<'a> = &'a mut T;
        fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
            self.0(registrar)
        }
    }
    impl<F> EffectLifetimeFixer0<F> {
        pub(super) const fn new<T>(f: F) -> Self
        where
            F: FnOnce(SideEffectRegistrar) -> &mut T,
        {
            Self(f)
        }
    }

    pub struct EffectLifetimeFixer1<F>(F);
    impl<T, F, R1> SideEffect for EffectLifetimeFixer1<F>
    where
        T: Send + 'static,
        F: FnOnce(SideEffectRegistrar) -> (&mut T, R1),
    {
        type Api<'a> = (&'a mut T, R1);
        fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
            self.0(registrar)
        }
    }
    impl<F> EffectLifetimeFixer1<F> {
        pub(super) const fn new<T, R1>(f: F) -> Self
        where
            F: FnOnce(SideEffectRegistrar) -> (&mut T, R1),
        {
            Self(f)
        }
    }

    pub struct EffectLifetimeFixer2<F>(F);
    impl<T, F, R1, R2> SideEffect for EffectLifetimeFixer2<F>
    where
        T: Send + 'static,
        F: FnOnce(SideEffectRegistrar) -> (&mut T, R1, R2),
    {
        type Api<'a> = (&'a mut T, R1, R2);
        fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
            self.0(registrar)
        }
    }
    impl<F> EffectLifetimeFixer2<F> {
        pub(super) const fn new<T, R1, R2>(f: F) -> Self
        where
            F: FnOnce(SideEffectRegistrar) -> (&mut T, R1, R2),
        {
            Self(f)
        }
    }
}

pub fn raw<T: Send + 'static>(
    initial: T,
) -> impl for<'a> SideEffect<
    Api<'a> = (
        &'a mut T,
        impl CData + Fn(Box<dyn FnOnce(&mut T)>),
        Arc<dyn Send + Sync + Fn(Box<dyn FnOnce()>)>,
    ),
> {
    EffectLifetimeFixer2::new(move |register: SideEffectRegistrar| register.raw(initial))
}

// NOTE: returns (), the no-op side effect
#[must_use]
pub fn as_listener() -> impl for<'a> SideEffect<Api<'a> = ()> {}

pub fn state<T: Send + 'static>(
    initial: T,
) -> impl for<'a> SideEffect<Api<'a> = (&'a mut T, impl CData + Fn(T))> {
    EffectLifetimeFixer1::new(move |register: SideEffectRegistrar| {
        let (state, rebuild, _) = register.raw(initial);
        let set_state = move |new_state| {
            rebuild(Box::new(|state| *state = new_state));
        };
        (state, set_state)
    })
}

#[allow(clippy::missing_panics_doc)] // false positive
pub fn lazy_state<T, F>(
    init: F,
) -> impl for<'a> SideEffect<Api<'a> = (&'a mut T, impl CData + Fn(T))>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    EffectLifetimeFixer1::new(move |register: SideEffectRegistrar| {
        let (cell, rebuild, _) = register.raw(Lazy::new(init));
        let set_state = move |new_state| {
            rebuild(Box::new(|cell| **cell = new_state));
        };
        (&mut **cell, set_state)
    })
}

pub fn value<T: Send + 'static>(value: T) -> impl for<'a> SideEffect<Api<'a> = &'a mut T> {
    EffectLifetimeFixer0::new(move |register: SideEffectRegistrar| register.raw(value).0)
}

#[allow(clippy::missing_panics_doc)] // false positive
pub fn lazy_value<T, F>(init: F) -> impl for<'a> SideEffect<Api<'a> = &'a mut T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    EffectLifetimeFixer0::new(move |register: SideEffectRegistrar| {
        let (cell, _, _) = register.raw(Lazy::new(init));
        &mut **cell
    })
}

#[must_use]
pub fn is_first_build() -> impl for<'a> SideEffect<Api<'a> = bool> {
    move |register: SideEffectRegistrar| {
        let has_built_before = register.register(value(false));
        let is_first_build = !*has_built_before;
        *has_built_before = true;
        is_first_build
    }
}

pub fn reducer<State, Action, Reducer>(
    reducer: Reducer,
    initial: State,
) -> impl for<'a> SideEffect<Api<'a> = (&'a mut State, impl CData + Fn(Action))>
where
    State: Send + 'static,
    Action: 'static,
    Reducer: Clone + Send + Sync + 'static + Fn(&State, Action) -> State,
{
    EffectLifetimeFixer1::new(move |register: SideEffectRegistrar| {
        let (state, update_state, _) = register.raw(initial);
        (state, move |action| {
            let reducer = reducer.clone();
            update_state(Box::new(move |state| *state = reducer(state, action)));
        })
    })
}

// NOTE: Commented out because:
// - This fails to compile due to a compiler bug (&'a mut R compiles fine, but &'a R doesn't)
// - I think people should really be using a hydrate equivalent instead of this
//   - A combo of lazy_value and run_on_change probably
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
