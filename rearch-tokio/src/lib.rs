#![allow(clippy::multiple_crate_versions)]
// TODO(GregoryConrad): remove multiple_crate_versions from allowlist once tokio deps are updated

use effects::{MutRef, StateTransformer};
use rearch::{CData, SideEffect, SideEffectRegistrar};
use rearch_effects as effects;
use std::{future::Future, sync::Arc};

struct FunctionalDrop<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Drop for FunctionalDrop<F> {
    fn drop(&mut self) {
        if let Some(callback) = core::mem::take(&mut self.0) {
            callback();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AsyncState<T> {
    Loading(Option<T>),
    Complete(T),
}

impl<T> AsyncState<T> {
    pub fn data(self) -> Option<T> {
        match self {
            Self::Loading(previous_data) => previous_data,
            Self::Complete(data) => Some(data),
        }
    }
}

/*
TODO I think this should be modified to return `impl 'a + FnMut(F) -> AsyncState<T>`
to remove the idle state
Also might want to consider cancelation too--maybe the same function should return a cancel token

#[must_use]
pub fn future<T, F>(
) -> impl for<'a> SideEffect<Api<'a> = (impl Fn() -> AsyncState<T> + 'a, impl FnMut(F) + 'a)>
where
    T: Clone + Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    move |register: SideEffectRegistrar<'a>| {
        let ((state, set_state), mut on_change) = register.register((
            effects::state(AsyncState::Idle(None)),
            effects::run_on_change(),
        ));
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
*/

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MutationState<T> {
    Idle(Option<T>),
    Loading(Option<T>),
    Complete(T),
}

impl<T> MutationState<T> {
    pub fn data(self) -> Option<T> {
        match self {
            Self::Idle(previous_data) | Self::Loading(previous_data) => previous_data,
            Self::Complete(data) => Some(data),
        }
    }

    pub fn map<U, F>(self, f: F) -> MutationState<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Idle(prev) => MutationState::Idle(prev.map(f)),
            Self::Loading(prev) => MutationState::Loading(prev.map(f)),
            Self::Complete(state) => MutationState::Complete(f(state)),
        }
    }

    pub fn as_mut(&mut self) -> MutationState<&mut T> {
        match *self {
            Self::Idle(ref mut prev) => MutationState::Idle(prev.as_mut()),
            Self::Loading(ref mut prev) => MutationState::Loading(prev.as_mut()),
            Self::Complete(ref mut state) => MutationState::Complete(state),
        }
    }
}

struct MutationLifetimeFixer<F, ST>(F, std::marker::PhantomData<ST>);
impl<F, ST, R1, R2> SideEffect for MutationLifetimeFixer<F, ST>
where
    F: FnOnce(SideEffectRegistrar) -> (MutationState<ST::Output<'_>>, R1, R2),
    ST: StateTransformer,
{
    type Api<'a> = (MutationState<ST::Output<'a>>, R1, R2);
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}
impl<F, ST> MutationLifetimeFixer<F, ST> {
    const fn new<R1, R2>(f: F) -> Self
    where
        F: FnOnce(SideEffectRegistrar) -> (MutationState<ST::Output<'_>>, R1, R2),
        ST: StateTransformer,
    {
        Self(f, std::marker::PhantomData)
    }
}

/// Allows you to trigger and cancel query mutations.
///
/// This should normally *not* be used with [`MutRef`].
#[must_use]
pub fn mutation<ST: StateTransformer, F>() -> impl for<'a> SideEffect<
    Api<'a> = (
        MutationState<ST::Output<'a>>,
        impl CData + Fn(F),
        impl CData + Fn(),
    ),
>
where
    F: Future<Output = ST::Input> + Send + 'static,
{
    MutationLifetimeFixer::<_, ST>::new(move |register: SideEffectRegistrar| {
        let ((state, mutate_state, run_txn), (_, on_change)) = register.register((
            effects::raw::<MutRef<MutationState<ST>>>(MutationState::Idle(None)),
            // This immitates run_on_change, but for external use (outside of build)
            effects::state::<MutRef<_>>(FunctionalDrop(None)),
        ));

        let state = state.as_mut().map(ST::as_output);
        let mutate = {
            let on_change = on_change.clone();
            let mutate_state = mutate_state.clone();
            let run_txn = Arc::clone(&run_txn);
            move |future| {
                let on_change = on_change.clone();
                let mutate_state = mutate_state.clone();
                run_txn(Box::new(move || {
                    mutate_state(Box::new(|state| {
                        let old_state = std::mem::replace(state, MutationState::Idle(None));
                        *state = MutationState::Loading(old_state.data());
                    }));

                    let mutate_state = mutate_state.clone();
                    let handle = tokio::spawn(async move {
                        let data = ST::from_input(future.await);
                        mutate_state(Box::new(move |state| {
                            *state = MutationState::Complete(data);
                        }));
                    });
                    on_change(FunctionalDrop(Some(move || handle.abort())));
                }));
            }
        };
        let clear = move || {
            let on_change = on_change.clone();
            let mutate_state = mutate_state.clone();
            run_txn(Box::new(move || {
                mutate_state(Box::new(|state| {
                    let old_state = std::mem::replace(state, MutationState::Idle(None));
                    *state = MutationState::Idle(old_state.data());
                }));
                on_change(FunctionalDrop(None)); // abort old future if present
            }));
        };
        (state, mutate, clear)
    })
}

/*
TODO this should probably be reworked to be hydrate-like instead of state-like

pub fn async_persist<T, R, Reader, Writer, ReadFuture, WriteFuture>(
    read: Reader,
    write: Writer,
) -> impl for<'a> SideEffect<Api<'a> = (AsyncPersistState<R>, impl FnMut(T) + Send + Sync + Clone)>
where
    T: Send + 'static,
    R: Clone + Send + 'static,
    Reader: FnOnce() -> ReadFuture + Send + 'static,
    Writer: Fn(T) -> WriteFuture + Send + Sync + 'static,
    ReadFuture: Future<Output = R> + Send + 'static,
    WriteFuture: Future<Output = R> + Send + 'static,
{
    move |register: SideEffectRegistrar| {
        let ((get_read, mut set_read), (write_state, set_write, _), is_first_build) =
            register.register((future(), mutation(), effects::is_first_build()));

        if is_first_build {
            set_read(read());
        }
        let state = match (write_state, get_read()) {
            (AsyncState::Idle(_), AsyncState::Loading(prev))
            | (AsyncState::Loading(prev @ Some(_)), _) => AsyncPersistState::Loading(prev),
            (AsyncState::Idle(_), AsyncState::Complete(data)) | (AsyncState::Complete(data), _) => {
                AsyncPersistState::Complete(data)
            }
            (AsyncState::Loading(None), read_state) => {
                AsyncPersistState::Loading(read_state.data())
            }
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
*/
