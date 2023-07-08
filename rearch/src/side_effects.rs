use std::{cell::OnceCell, sync::Arc};

use crate::{SideEffect, SideEffectRegistrar};

pub fn state<'a, T: Send + 'static>(
    initial: T,
) -> impl SideEffect<'a, Api = (&'a mut T, impl Fn(T) + Clone + Send + Sync)> {
    move |register: SideEffectRegistrar<'a>| {
        let (state, rebuild) = register.raw(initial);
        let set_state = move |new_state| {
            rebuild(Box::new(|state| *state = new_state));
        };
        (state, set_state)
    }
}

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
pub fn lazy_state<'a, T, F>(
    init: F,
) -> impl SideEffect<'a, Api = (&'a mut T, impl Fn(T) + Clone + Send + Sync)>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
        let ((cell, f), rebuild) = register.raw((OnceCell::new(), Some(init)));
        cell.get_or_init(|| std::mem::take(f).expect("Init fn should be present for cell init")());
        let state = cell.get_mut().expect("State initialized above");
        let set_state = move |new_state| {
            rebuild(Box::new(|effect| {
                effect.0.take();
                _ = effect.0.set(new_state);
            }));
        };
        (state, set_state)
    }
}

pub fn value<'a, T: Send + 'static>(value: T) -> impl SideEffect<'a, Api = &'a mut T> {
    move |register: SideEffectRegistrar<'a>| {
        let (state, _) = register.raw(value);
        state
    }
}

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
pub fn lazy_value<'a, T, F>(init: F) -> impl SideEffect<'a, Api = &'a mut T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
        let ((cell, f), _) = register.raw((OnceCell::new(), Some(init)));
        cell.get_or_init(|| std::mem::take(f).expect("Init fn should be present for cell init")());
        cell.get_mut().expect("State initialized above")
    }
}

#[must_use]
pub fn is_first_build<'a>() -> impl SideEffect<'a, Api = bool> {
    move |register: SideEffectRegistrar<'a>| {
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
pub fn rebuilder<'a>() -> impl SideEffect<'a, Api = impl Fn() + Clone + Send + Sync> {
    move |register: SideEffectRegistrar<'a>| {
        let ((), rebuild) = register.raw(());
        move || rebuild(Box::new(|_| {}))
    }
}

pub fn run_once<'a, F>(f: F) -> impl SideEffect<'a, Api = ()>
where
    F: FnOnce() + Send + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
        register.register(lazy_value(f));
    }
}

/// Side effect that runs a callback whenever it changes and is dropped.
/// Similar to `useEffect` from React.
pub fn run_on_change<'a, F>() -> impl SideEffect<'a, Api = impl FnMut(F) + 'a>
where
    F: FnOnce() + Send + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
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

/// A thin wrapper around the state side effect that enables easy state persistence.
///
/// You provide a `read` function and a `write` function,
/// and you receive the status of the latest read/write operation,
/// in addition to a persist function that persists new state and triggers rebuilds.
///
/// Note: when possible, it is highly recommended to use async persist instead of sync persist.
/// This effect is blocking, which will prevent other capsule updates.
/// However, this function is perfect for quick I/O, like when using something similar to redb.
pub fn sync_persist<'a, Read, Write, R, T>(
    read: Read,
    write: Write,
) -> impl SideEffect<'a, Api = (&'a R, impl Fn(T) + Clone + Send + Sync)>
where
    T: Send + 'static,
    R: Send + 'static,
    Read: FnOnce() -> R + Send + 'static,
    Write: Fn(T) -> R + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
        let ((state, set_state), write) =
            register.register((lazy_state(read), value(Arc::new(write))));

        let write = Arc::clone(write);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };

        (&*state, persist)
    }
}

// TODO convert below side effects too
/*
#[cfg(feature = "tokio-side-effects")]
fn future_from_fn<R, Future>(
    &mut self,
    future: impl FnOnce() -> Future,
    dependencies: impl DependencyList,
) -> AsyncState<R>
where
    R: Send + Sync + 'static,
    Future: std::future::Future<Output = R> + Send + 'static,
{
    let rebuild = self.rebuilder();
    let (state, set_state) = self.rebuildless_state(|| AsyncState::Loading(None));

    self.effect(
        || {
            let curr_data = state.data();
            set_state(AsyncState::Loading(curr_data));

            let future = future();
            let handle = tokio::task::spawn(async move {
                let data = future.await;
                rebuild(move || {
                    set_state(AsyncState::Complete(Arc::new(data)));
                });
            });

            move || handle.abort()
        },
        dependencies,
    );

    state.as_ref().clone()
}

#[cfg(feature = "tokio-side-effects")]
fn future<R, Future>(
    &mut self,
    future: Future,
    dependencies: impl DependencyList,
) -> AsyncState<R>
where
    R: Send + Sync + 'static,
    Future: std::future::Future<Output = R> + Send + 'static,
{
    self.future_from_fn(|| future, dependencies)
}

#[cfg(feature = "tokio-side-effects")]
fn async_persist<T, R, Reader, Writer, ReadFuture, WriteFuture>(
    &mut self,
    read: Reader,
    write: Writer,
) -> (AsyncState<R>, impl FnMut(T) + Send + Sync + Clone + 'static)
where
    T: Send + 'static,
    R: Send + Sync + 'static,
    Reader: FnOnce() -> ReadFuture + Send + 'static,
    Writer: FnOnce(T) -> WriteFuture + Send + 'static,
    ReadFuture: std::future::Future<Output = R> + Send + 'static,
    WriteFuture: std::future::Future<Output = R> + Send + 'static,
{
    let data_to_persist_mutex = self.callonce(|| Mutex::new(None::<T>));
    let data_to_persist = {
        let mut data_to_persist = data_to_persist_mutex
            .lock()
            .expect("Mutex shouldn't fail to lock");
        std::mem::take(&mut *data_to_persist)
    };

    let rebuild = self.rebuilder();
    let persist = move |new_data| {
        let data_to_persist_mutex = data_to_persist_mutex.clone();
        rebuild(move || {
            let mut data_to_persist = data_to_persist_mutex
                .lock()
                .expect("Mutex shouldn't fail to lock");
            *data_to_persist = Some(new_data);
        })
    };

    // Deps changes whenever new data is persisted so that self.future_from_fn will
    // always have the most up to date future
    let deps_mutex = self.callonce(|| Mutex::new(false));
    let deps = {
        let mut deps = deps_mutex.lock().expect("Mutex shouldn't fail to lock");
        if data_to_persist.is_some() {
            *deps = !*deps;
        }
        (*deps,)
    };

    let future = async move {
        match data_to_persist {
            Some(data_to_persist) => write(data_to_persist).await,
            None => read().await, // this will only actually be called on first build
        }
    };

    let state = self.future(future, deps);

    (state, persist)
}

fn reducer_from_fn<State, Action, Reducer>(
    &mut self,
    reducer: Reducer,
    initial_state: impl FnOnce() -> State,
) -> (Arc<State>, impl Fn(Action) + Send + Sync + 'static)
where
    State: Send + Sync + 'static,
    Reducer: Fn(&State, Action) -> State + Send + Sync + 'static,
{
    let (state, set_state) = self.state_from_fn(initial_state);
    let dispatch = {
        let state = state.clone();
        move |action| {
            set_state(reducer(&state, action));
        }
    };
    (state, dispatch)
}

fn reducer<State, Action, Reducer>(
    &mut self,
    reducer: Reducer,
    initial_state: State,
) -> (Arc<State>, impl Fn(Action) + Send + Sync + 'static)
where
    State: Send + Sync + 'static,
    Reducer: Fn(&State, Action) -> State + Send + Sync + 'static,
{
    self.reducer_from_fn(reducer, || initial_state)
}

fn rebuildless_nonce(&mut self) -> (u16, impl Fn() + Send + Sync + Clone + 'static) {
    let (nonce, set_nonce) = self.rebuildless_state(|| 0u16);
    (*nonce, move || set_nonce(nonce.overflowing_add(1).0))
}

#[cfg(feature = "tokio-side-effects")]
fn mutation<T, Mutation, Future>(
    &mut self,
    mutation: Mutation,
) -> (
    AsyncMutationState<T>,
    impl Fn() + Send + Sync + Clone + 'static,
    impl Fn() + Send + Sync + Clone + 'static,
)
where
    T: Send + Sync + 'static,
    Mutation: FnOnce() -> Future + Send + 'static,
    Future: std::future::Future<Output = T> + Send + 'static,
{
    let (state, set_state) = self.rebuildless_state(|| AsyncMutationState::Idle(None));
    let (nonce, increment_nonce) = self.rebuildless_nonce();
    let (active, set_active) = {
        let (active, set_active) = self.rebuildless_state(|| false);
        (*active, set_active)
    };

    let curr_data = state.data();
    if !active {
        set_state(AsyncMutationState::Idle(curr_data.clone()));
    }

    let rebuild = self.rebuilder();
    self.effect(
        || {
            let handle = active.then(move || {
                set_state(AsyncMutationState::Loading(curr_data));
                tokio::task::spawn(async move {
                    let data = mutation().await;
                    rebuild(move || {
                        set_state(AsyncMutationState::Complete(Arc::new(data)));
                    });
                })
            });

            move || {
                if let Some(handle) = handle {
                    handle.abort()
                }
            }
        },
        (nonce, active),
    );

    let rebuild = self.rebuilder();
    let mutate = {
        let set_active = set_active.clone();
        let increment_nonce = increment_nonce.clone();
        move || {
            let set_active = set_active.clone();
            let increment_nonce = increment_nonce.clone();
            rebuild(move || {
                set_active(true);
                increment_nonce();
            })
        }
    };

    let rebuild = self.rebuilder();
    let clear = move || {
        let set_active = set_active.clone();
        let increment_nonce = increment_nonce.clone();
        rebuild(move || {
            set_active(false);
            increment_nonce();
        })
    };

    (state.as_ref().clone(), mutate, clear)
}

#[cfg(feature = "tokio-side-effects")]
pub use async_state::*;

#[cfg(feature = "tokio-side-effects")]
mod async_state {
    use std::sync::Arc;

    pub enum AsyncState<T> {
        Loading(Option<Arc<T>>),
        Complete(Arc<T>),
    }

    impl<T> AsyncState<T> {
        pub fn data(&self) -> Option<Arc<T>> {
            match self {
                Self::Loading(previous_data) => previous_data.clone(),
                Self::Complete(data) => Some(data.clone()),
            }
        }
    }

    impl<T> Clone for AsyncState<T> {
        fn clone(&self) -> Self {
            match self {
                Self::Loading(previous_data) => Self::Loading(previous_data.clone()),
                Self::Complete(data) => Self::Complete(data.clone()),
            }
        }
    }

    pub enum AsyncMutationState<T> {
        Idle(Option<Arc<T>>),
        Loading(Option<Arc<T>>),
        Complete(Arc<T>),
    }

    impl<T> AsyncMutationState<T> {
        pub fn data(&self) -> Option<Arc<T>> {
            match self {
                Self::Idle(previous_data) => previous_data.clone(),
                Self::Loading(previous_data) => previous_data.clone(),
                Self::Complete(data) => Some(data.clone()),
            }
        }
    }

    impl<T> Clone for AsyncMutationState<T> {
        fn clone(&self) -> Self {
            match self {
                Self::Idle(previous_data) => Self::Idle(previous_data.clone()),
                Self::Loading(previous_data) => Self::Loading(previous_data.clone()),
                Self::Complete(data) => Self::Complete(data.clone()),
            }
        }
    }
}
*/
