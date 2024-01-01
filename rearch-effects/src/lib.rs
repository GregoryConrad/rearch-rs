use rearch::{CData, SideEffect, SideEffectRegistrar};
use std::{cell::OnceCell, sync::Arc};

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

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
#[allow(clippy::missing_panics_doc)] // false positive
pub fn lazy_state<T, F>(
    init: F,
) -> impl for<'a> SideEffect<Api<'a> = (&'a mut T, impl CData + Fn(T))>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    EffectLifetimeFixer1::new(move |register: SideEffectRegistrar| {
        let ((cell, f), rebuild, _) = register.raw((OnceCell::new(), Some(init)));
        cell.get_or_init(|| std::mem::take(f).expect("Init fn should be present for cell init")());
        let state = cell.get_mut().expect("State initialized above");
        let set_state = move |new_state| {
            rebuild(Box::new(|effect| {
                effect.0.take();
                _ = effect.0.set(new_state);
            }));
        };
        (state, set_state)
    })
}

pub fn value<T: Send + 'static>(value: T) -> impl for<'a> SideEffect<Api<'a> = &'a mut T> {
    EffectLifetimeFixer0::new(move |register: SideEffectRegistrar| register.raw(value).0)
}

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
#[allow(clippy::missing_panics_doc)] // false positive
pub fn lazy_value<T, F>(init: F) -> impl for<'a> SideEffect<Api<'a> = &'a mut T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    EffectLifetimeFixer0::new(move |register: SideEffectRegistrar| {
        let ((cell, f), _, _) = register.raw((OnceCell::new(), Some(init)));
        cell.get_or_init(|| std::mem::take(f).expect("Init fn should be present for cell init")());
        cell.get_mut().expect("State initialized above")
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

/// A side effect that provides a function that triggers rebuilds.
/// YOU SHOULD ALMOST NEVER USE THIS SIDE EFFECT!
/// Only use this side effect when you don't have any state that you wish to update in the rebuild
/// (which is extremely rare).
#[must_use]
pub fn rebuilder() -> impl for<'a> SideEffect<Api<'a> = impl CData + Fn()> {
    move |register: SideEffectRegistrar| {
        let ((), rebuild, _) = register.raw(());
        move || rebuild(Box::new(|()| {}))
    }
}

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

pub fn reducer<State, Action, Reducer>(
    reducer: Reducer,
    initial: State,
) -> impl for<'a> SideEffect<Api<'a> = (State, impl CData + Fn(Action))>
where
    State: Clone + Send + Sync + 'static,
    Reducer: Fn(State, Action) -> State + Clone + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar<'_>| {
        let (state, set_state) = register.register(state(initial));
        (state.clone(), {
            let state = state.clone();
            move |action| {
                let state = state.clone();
                set_state(reducer(state, action));
            }
        })
    }
}

pub fn lazy_reducer<State, Action, Reducer>(
    reducer: Reducer,
    initial: impl FnOnce() -> State + Send + 'static,
) -> impl for<'a> SideEffect<Api<'a> = (State, impl CData + Fn(Action))>
where
    State: Clone + Send + Sync + 'static,
    Reducer: Fn(State, Action) -> State + Clone + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar| {
        let (state, set_state) = register.register(lazy_state(initial));
        (state.clone(), {
            let state = state.clone();
            move |action| {
                let state = state.clone();
                set_state(reducer(state, action));
            }
        })
    }
}

// TODO should this actually be hydrate like in Dart?
/// A thin wrapper around the state side effect that enables easy state persistence.
///
/// You provide a `read` function and a `write` function,
/// and you receive the status of the latest read/write operation,
/// in addition to a persist function that persists new state and triggers rebuilds.
///
/// Note: when possible, it is highly recommended to use async persist instead of sync persist.
/// This effect is blocking, which will prevent other capsule updates.
/// However, this function is perfect for quick I/O, like when using something similar to redb.
pub fn sync_persist<Read, Write, R, T>(
    read: Read,
    write: Write,
) -> impl for<'a> SideEffect<Api<'a> = (&'a R, impl CData + Fn(T))>
where
    T: Send + 'static,
    R: Send + 'static,
    Read: FnOnce() -> R + Send + 'static,
    Write: Fn(T) -> R + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar| {
        let (state, set_state) = register.register(lazy_state(read));
        let write = Arc::new(write);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };
        (&*state, persist)
    }
}
*/
