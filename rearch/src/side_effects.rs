use std::{cell::OnceCell, marker::PhantomData, sync::Arc};

use crate::{SideEffect, SideEffectRebuilder};

// Note: We use Arc/Box<dyn Fn()> extensively throughout the SideEffect::Apis in order to:
// - Improve users' testability (it is difficult to mock static dispatch from SideEffect::Apis)
// - Improve performance (somehow Arc performed slightly better than static dispatch on benchmarks)
// - Avoid yet another nightly requirement (feature gate `impl_trait_in_assoc_type`)

type Rebuilder<T> = Box<dyn SideEffectRebuilder<T>>;

pub struct StateEffect<T>(T);
impl<T> StateEffect<T> {
    pub const fn new(default: T) -> Self {
        Self(default)
    }
}
impl<T: Send + 'static> SideEffect for StateEffect<T> {
    type Api<'a> = (&'a mut T, Arc<dyn Fn(T) + Send + Sync>);

    fn api(&mut self, rebuild: Rebuilder<Self>) -> Self::Api<'_> {
        (
            &mut self.0,
            Arc::new(move |new_state| {
                rebuild(Box::new(|effect| effect.0 = new_state));
            }),
        )
    }
}

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
pub struct LazyStateEffect<T, F: FnOnce() -> T>(OnceCell<T>, Option<F>);
impl<T, F: FnOnce() -> T> LazyStateEffect<T, F> {
    pub const fn new(default: F) -> Self {
        Self(OnceCell::new(), Some(default))
    }
}
impl<T, F> SideEffect for LazyStateEffect<T, F>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    type Api<'a> = (&'a mut T, Arc<dyn Fn(T) + Send + Sync>);

    fn api(&mut self, rebuild: Rebuilder<Self>) -> Self::Api<'_> {
        self.0.get_or_init(|| {
            std::mem::take(&mut self.1).expect("Init fn should be present for state init")()
        });
        (
            self.0.get_mut().expect("'State initialized above"),
            Arc::new(move |new_state| {
                rebuild(Box::new(|effect| {
                    effect.0.take();
                    _ = effect.0.set(new_state);
                }));
            }),
        )
    }
}

pub struct ValueEffect<T>(T);
impl<T> ValueEffect<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}
impl<T> SideEffect for ValueEffect<T>
where
    T: Send + 'static,
{
    type Api<'a> = &'a mut T;

    fn api(&mut self, _: Rebuilder<Self>) -> Self::Api<'_> {
        &mut self.0
    }
}

// This uses a hacked together Lazy implementation because LazyCell doesn't have force_mut;
// see https://github.com/rust-lang/rust/issues/109736#issuecomment-1605787094
pub struct LazyValueEffect<T, F: FnOnce() -> T>(OnceCell<T>, Option<F>);
impl<T, F: FnOnce() -> T> LazyValueEffect<T, F> {
    pub const fn new(init: F) -> Self {
        Self(OnceCell::new(), Some(init))
    }
}
impl<T, F> SideEffect for LazyValueEffect<T, F>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    type Api<'a> = &'a mut T;

    fn api(&mut self, _: Rebuilder<Self>) -> Self::Api<'_> {
        self.0.get_or_init(|| {
            std::mem::take(&mut self.1).expect("Init fn should be present for state init")()
        });
        self.0.get_mut().expect("State initialized above")
    }
}

/// A side effect that provides a function that triggers rebuilds.
/// YOU SHOULD ALMOST NEVER USE THIS SIDE EFFECT!
/// Only use this side effect when you don't have any state that you wish to update in the rebuild
/// (which is extremely rare).
#[derive(Default)]
pub struct RebuilderEffect;
impl RebuilderEffect {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
impl SideEffect for RebuilderEffect {
    type Api<'a> = Arc<dyn Fn() + Send + Sync>;

    fn api(&mut self, rebuild: Rebuilder<Self>) -> Self::Api<'_> {
        Arc::new(move || rebuild(Box::new(|_| {})))
    }
}

pub type RunOnceEffect<F> = LazyValueEffect<(), F>;

/// Side effect that runs a callback whenever it changes and is dropped.
/// Similar to `useEffect` from React.
pub struct RunOnChangeEffect<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Default for RunOnChangeEffect<F> {
    fn default() -> Self {
        Self(None)
    }
}
impl<F: FnOnce()> RunOnChangeEffect<F> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl<F: FnOnce()> Drop for RunOnChangeEffect<F> {
    fn drop(&mut self) {
        if let Some(callback) = std::mem::take(&mut self.0) {
            callback();
        }
    }
}
impl<F> SideEffect for RunOnChangeEffect<F>
where
    F: FnOnce() + Send + 'static,
{
    type Api<'a> = Box<dyn FnMut(F) + 'a>;

    fn api(&mut self, _: Rebuilder<Self>) -> Self::Api<'_> {
        Box::new(move |new_effect| {
            if let Some(callback) = std::mem::take(&mut self.0) {
                callback();
            }
            self.0 = Some(new_effect);
        })
    }
}

#[derive(Default)]
pub struct IsFirstBuildEffect {
    has_built: bool,
}
impl IsFirstBuildEffect {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl SideEffect for IsFirstBuildEffect {
    type Api<'a> = bool;

    fn api(&mut self, _: Rebuilder<Self>) -> Self::Api<'_> {
        let is_first_build = !self.has_built;
        self.has_built = true;
        is_first_build
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
pub struct SyncPersistEffect<Read: FnOnce() -> R, Write, R, T> {
    data: (LazyStateEffect<R, Read>, ValueEffect<Arc<Write>>),
    ghost: PhantomData<T>,
}
impl<Read: FnOnce() -> R, Write, R, T> SyncPersistEffect<Read, Write, R, T> {
    pub fn new(read: Read, write: Write) -> Self {
        Self {
            data: (
                LazyStateEffect::new(read),
                ValueEffect::new(Arc::new(write)),
            ),
            ghost: PhantomData,
        }
    }
}
impl<Read, Write, R, T> SideEffect for SyncPersistEffect<Read, Write, R, T>
where
    T: Send + 'static,
    R: Send + 'static,
    Read: FnOnce() -> R + Send + 'static,
    Write: Fn(T) -> R + Send + Sync + 'static,
{
    type Api<'a> = (&'a R, Arc<dyn Fn(T) + Send + Sync>);

    fn api(&mut self, rebuild: Rebuilder<Self>) -> Self::Api<'_> {
        let ((state, set_state), write) = self.data.api(Box::new(move |mutation| {
            rebuild(Box::new(move |effect| mutation(&mut effect.data)));
        }));

        let write = Arc::clone(write);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };

        (state, Arc::new(persist))
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
