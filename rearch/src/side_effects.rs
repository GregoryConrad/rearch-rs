use std::{cell::OnceCell, sync::Arc};

use crate::{SideEffect, SideEffectRegistrar};

pub fn raw<'a, T: Send + 'static>(
    initial: T,
) -> impl SideEffect<
    'a,
    Api = (
        &'a mut T,
        impl Fn(Box<dyn FnOnce(&mut T)>) + Clone + Send + Sync,
    ),
> {
    move |register: SideEffectRegistrar<'a>| register.raw(initial)
}

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
#[must_use]
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

pub fn reducer<'a, State, Action, Reducer>(
    reducer: Reducer,
    initial: State,
) -> impl SideEffect<'a, Api = (State, impl Fn(Action) + Clone + Send + Sync)>
where
    State: Clone + Send + Sync + 'static,
    Reducer: Fn(State, Action) -> State + Clone + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
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

pub fn lazy_reducer<'a, State, Action, Reducer>(
    reducer: Reducer,
    initial: impl FnOnce() -> State + Send + 'static,
) -> impl SideEffect<'a, Api = (State, impl Fn(Action) + Clone + Send + Sync)>
where
    State: Clone + Send + Sync + 'static,
    Reducer: Fn(State, Action) -> State + Clone + Send + Sync + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
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
        let (state, set_state) = register.register(lazy_state(read));
        let write = Arc::new(write);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };
        (&*state, persist)
    }
}

#[cfg(feature = "tokio-side-effects")]
pub use tokio_side_effects::*;

#[cfg(feature = "tokio-side-effects")]
mod tokio_side_effects {
    use std::{cell::RefCell, future::Future, rc::Rc};

    use super::*;

    #[derive(Clone)]
    pub enum AsyncState<T> {
        Idle(Option<T>),
        Loading(Option<T>),
        Complete(T),
    }

    impl<T> AsyncState<T> {
        pub fn data(self) -> Option<T> {
            match self {
                Self::Idle(previous_data) => previous_data,
                Self::Loading(previous_data) => previous_data,
                Self::Complete(data) => Some(data),
            }
        }
    }

    #[derive(Clone)]
    pub enum AsyncPersistState<T> {
        Loading(Option<T>),
        Complete(T),
    }

    impl<T> AsyncPersistState<T> {
        pub fn data(self) -> Option<T> {
            match self {
                Self::Loading(previous_data) => previous_data,
                Self::Complete(data) => Some(data),
            }
        }
    }

    pub fn future<'a, T, F>(
    ) -> impl SideEffect<'a, Api = (impl Fn() -> AsyncState<T> + 'a, impl FnMut(F) + 'a)>
    where
        T: Clone + Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        move |register: SideEffectRegistrar<'a>| {
            let ((state, set_state), mut on_change) =
                register.register((state(AsyncState::Idle(None)), run_on_change()));
            let state = Rc::new(RefCell::new(state));
            let get = {
                let state = Rc::clone(&state);
                move || state.borrow().clone()
            };
            let set = move |future| {
                let mut state = state.borrow_mut();
                let old_state = std::mem::replace(*state, AsyncState::Idle(None));
                **state = AsyncState::Loading(old_state.data());

                let set_state = set_state.clone();
                let handle = tokio::spawn(async move {
                    let data = future.await;
                    set_state(AsyncState::Complete(data));
                });
                on_change(move || handle.abort());
            };
            (get, set)
        }
    }

    pub fn mutation<'a, T, F>() -> impl SideEffect<
        'a,
        Api = (
            AsyncState<T>,
            impl Fn(F) + Clone + Send + Sync,
            impl Fn() + Clone + Send + Sync,
        ),
    >
    where
        T: Clone + Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        move |register: SideEffectRegistrar<'a>| {
            let ((state, rebuild), (_, on_change)) = register.register((
                raw(AsyncState::Idle(None)),
                // This immitates run_on_change, but for external use (outside of build)
                state(FunctionalDrop(None)),
            ));

            let state = state.clone();
            let mutate = {
                let rebuild = rebuild.clone();
                move |future| {
                    rebuild(Box::new(|state| {
                        let old_state = std::mem::replace(state, AsyncState::Idle(None));
                        *state = AsyncState::Loading(old_state.data());
                    }));

                    let rebuild = rebuild.clone();
                    let handle = tokio::spawn(async move {
                        let data = future.await;
                        rebuild(Box::new(move |state| {
                            *state = AsyncState::Complete(data);
                        }));
                    });
                    on_change(FunctionalDrop(Some(move || handle.abort())));
                }
            };
            let clear = move || {
                rebuild(Box::new(|state| {
                    let old_state = std::mem::replace(state, AsyncState::Idle(None));
                    *state = AsyncState::Idle(old_state.data());
                }));
            };
            (state, mutate, clear)
        }
    }

    pub fn async_persist<'a, T, R, Reader, Writer, ReadFuture, WriteFuture>(
        read: Reader,
        write: Writer,
    ) -> impl SideEffect<'a, Api = (AsyncPersistState<R>, impl FnMut(T) + Send + Sync + Clone)>
    where
        T: Send + 'static,
        R: Clone + Send + 'static,
        Reader: FnOnce() -> ReadFuture + Send + 'static,
        Writer: Fn(T) -> WriteFuture + Send + Sync + 'static,
        ReadFuture: Future<Output = R> + Send + 'static,
        WriteFuture: Future<Output = R> + Send + 'static,
    {
        move |register: SideEffectRegistrar<'a>| {
            let ((get_read, mut set_read), (write_state, set_write, _), is_first_build) =
                register.register((future(), mutation(), is_first_build()));

            if is_first_build {
                set_read(read());
            }
            let state = match (write_state, get_read()) {
                (AsyncState::Idle(_), AsyncState::Loading(prev)) => {
                    AsyncPersistState::Loading(prev)
                }
                (AsyncState::Idle(_), AsyncState::Complete(data)) => {
                    AsyncPersistState::Complete(data)
                }

                (AsyncState::Loading(None), read_state) => {
                    AsyncPersistState::Loading(read_state.data())
                }
                (AsyncState::Loading(prev @ Some(_)), _) => AsyncPersistState::Loading(prev),

                (AsyncState::Complete(data), _) => AsyncPersistState::Complete(data),

                (_, AsyncState::Idle(_)) => {
                    unreachable!("Read should never be idle")
                }
            };

            let write = Arc::new(write);
            let persist = move |new_data| {
                let write = Arc::clone(&write);
                set_write(async move { write(new_data).await });
            };

            (state, persist)
        }
    }
}
