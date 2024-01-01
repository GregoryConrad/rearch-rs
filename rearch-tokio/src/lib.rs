use std::future::Future;

use rearch::{SideEffect, SideEffectRegistrar};
use rearch_effects as effects;

struct FunctionalDrop<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Drop for FunctionalDrop<F> {
    fn drop(&mut self) {
        if let Some(callback) = std::mem::take(&mut self.0) {
            callback();
        }
    }
}

#[derive(Clone)]
pub enum AsyncState<T> {
    Idle(Option<T>),
    Loading(Option<T>),
    Complete(T),
}

impl<T> AsyncState<T> {
    pub fn data(self) -> Option<T> {
        match self {
            Self::Idle(previous_data) | Self::Loading(previous_data) => previous_data,
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

/*
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

#[must_use]
pub fn mutation<T, F>() -> impl for<'a> SideEffect<
    Api<'a> = (
        AsyncState<T>,
        impl Fn(F) + Clone + Send + Sync,
        impl Fn() + Clone + Send + Sync,
    ),
>
where
    T: Clone + Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    move |register: SideEffectRegistrar| {
        let ((state, rebuild, _), (_, on_change)) = register.register((
            effects::raw(AsyncState::Idle(None)),
            // This immitates run_on_change, but for external use (outside of build)
            effects::state(FunctionalDrop(None)),
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

/*
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
