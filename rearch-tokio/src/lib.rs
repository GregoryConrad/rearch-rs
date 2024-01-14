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
}

#[must_use]
pub fn mutation<T, F>(
) -> impl for<'a> SideEffect<Api<'a> = (&'a MutationState<T>, impl CData + Fn(F), impl CData + Fn())>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    RefEffectLifetimeFixer2::new(move |register: SideEffectRegistrar| {
        let ((state, mutate_state, run_txn), (_, on_change)) = register.register((
            effects::raw(MutationState::Idle(None)),
            // This immitates run_on_change, but for external use (outside of build)
            effects::state(FunctionalDrop(None)),
        ));

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
                        let data = future.await;
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
        (&*state, mutate, clear)
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

struct RefEffectLifetimeFixer2<F>(F);
impl<T, F, R1, R2> SideEffect for RefEffectLifetimeFixer2<F>
where
    T: Send + 'static,
    F: FnOnce(SideEffectRegistrar) -> (&T, R1, R2),
{
    type Api<'a> = (&'a T, R1, R2);
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}
impl<F> RefEffectLifetimeFixer2<F> {
    const fn new<T, R1, R2>(f: F) -> Self
    where
        F: FnOnce(SideEffectRegistrar) -> (&T, R1, R2),
    {
        Self(f)
    }
}
