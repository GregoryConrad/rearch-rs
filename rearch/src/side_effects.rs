use std::sync::{Arc, Mutex};

use crate::{SideEffectHandle, SideEffectHandleApi};

pub trait BuiltinSideEffects {
    type Api: SideEffectHandleApi + 'static;

    fn register_side_effect<R: Send + Sync + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut Self::Api) -> R,
    ) -> Arc<R>;

    fn callonce<R: Send + Sync + 'static>(&mut self, callback: impl FnOnce() -> R) -> Arc<R> {
        self.register_side_effect(|_| callback())
    }

    fn rebuilder<Mutation: FnOnce() + 'static>(
        &mut self,
    ) -> Arc<impl Fn(Mutation) + Send + Sync + 'static> {
        self.register_side_effect(|api| api.rebuilder())
    }

    fn rebuildless_state<T: Send + Sync + 'static>(
        &mut self,
        default: impl FnOnce() -> T,
    ) -> (Arc<T>, impl Fn(T) + Send + Sync + Clone + 'static) {
        let mutex = self.callonce(|| Mutex::new(Arc::new(default())));

        let curr_state = mutex.lock().expect("Mutex shouldn't fail to lock").clone();
        let set_state = move |new_state| {
            let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");
            *state = Arc::new(new_state);
        };

        (curr_state, set_state)
    }

    fn state_from_fn<T: Send + Sync + 'static>(
        &mut self,
        default: impl FnOnce() -> T,
    ) -> (Arc<T>, impl Fn(T) + Send + Sync + Clone + 'static) {
        let rebuild = self.rebuilder();
        let (state, set_state) = self.rebuildless_state(default);

        let set_state = move |new_state| {
            let set_state = set_state.clone();
            rebuild(move || set_state(new_state))
        };

        (state, set_state)
    }

    fn state_from_default<T: Send + Sync + Default + 'static>(
        &mut self,
    ) -> (Arc<T>, impl Fn(T) + Send + Sync + Clone + 'static) {
        self.state_from_fn(T::default)
    }

    fn state<T: Send + Sync + 'static>(
        &mut self,
        default: T,
    ) -> (Arc<T>, impl Fn(T) + Send + Sync + Clone + 'static) {
        self.state_from_fn(|| default)
    }

    // TODO should we call the last disposal when the capsule itself is disposed?
    fn effect<DL, OnDispose, Effect>(&mut self, effect: Effect, dependencies: DL)
    where
        DL: DependencyList,
        OnDispose: FnOnce() + Send + 'static,
        Effect: FnOnce() -> OnDispose,
    {
        let state = self.callonce(|| Mutex::new(None::<(DL, _)>));
        let mut state = state.lock().expect("Mutex shouldn't fail to lock");
        match &mut *state {
            None => *state = Some((dependencies, effect())),
            Some((curr_deps, on_dispose)) if !curr_deps.eq(&dependencies) => {
                // We need to grab ownership of the old on dispose in order to call it
                // (since it is an FnOnce), so we need std::mem::replace() to swap in the new one
                std::mem::replace(on_dispose, effect())();
                *curr_deps = dependencies;
            }
            Some(_) => (),
        }
    }

    fn memo<DL: DependencyList, R: Send + Sync + 'static>(
        &mut self,
        memo: impl FnOnce() -> R,
        dependencies: DL,
    ) -> Arc<R> {
        let (state, set_state) = self.rebuildless_state(|| None::<(DL, Arc<R>)>);
        match state.as_ref() {
            Some((curr_deps, curr_state)) if curr_deps.eq(&dependencies) => curr_state.clone(),
            _ => {
                let data = Arc::new(memo());
                set_state(Some((dependencies, data.clone())));
                data
            }
        }
    }

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

    /// A thin wrapper around the state side effect that enables easy state persistence.
    ///
    /// You provide a `read` function and a `write` function,
    /// and you receive of status of the latest read/write operation,
    /// in addition to a persist function that persists new state and triggers rebuilds.
    ///
    /// Note: when possible, it is highly recommended to use async_persist instead of sync_persist.
    /// This function is blocking, which will prevent other capsule updates.
    /// However, this function is perfect for quick I/O, like when using something similar to redb.
    fn sync_persist<T, R: Send + Sync + 'static>(
        &mut self,
        read: impl FnOnce() -> R,
        write: impl Fn(T) -> R + Send + Sync + 'static,
    ) -> (Arc<R>, impl Fn(T) + Send + Sync + 'static) {
        let (state, set_state) = self.state_from_fn(read);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };
        (state, persist)
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

    // TODO self.mutation
}

impl<Handle: SideEffectHandle> BuiltinSideEffects for Handle {
    type Api = Handle::Api;
    fn register_side_effect<R: Send + Sync + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut Self::Api) -> R,
    ) -> Arc<R> {
        SideEffectHandle::register_side_effect(self, side_effect)
    }
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
                AsyncState::Loading(previous) => previous.clone(),
                AsyncState::Complete(data) => Some(data.clone()),
            }
        }
    }

    impl<T> Clone for AsyncState<T> {
        fn clone(&self) -> Self {
            match self {
                AsyncState::Loading(previous_data) => AsyncState::Loading(previous_data.clone()),
                AsyncState::Complete(data) => AsyncState::Complete(data.clone()),
            }
        }
    }
}

pub trait DependencyList: Send + Sync + 'static {
    fn eq(&self, other: &Self) -> bool;
}

impl<DL: DependencyList> DependencyList for Arc<DL> {
    fn eq(&self, other: &Self) -> bool {
        let s: &DL = self;
        s.eq(other)
    }
}

macro_rules! generate_dep_list_impl {
    ($($type:ident),*) => {
        paste::paste! {
            impl<$($type: Eq + Send + Sync + 'static),*> DependencyList for ($($type,)*) {
                #[allow(non_snake_case, unused_parens)]
                fn eq(&self, other: &Self) -> bool {
                    let ($([<s_ $type>]),*) = self;
                    let ($([<o_ $type>]),*) = other;
                    true $(&& [<s_ $type>] == [<o_ $type>])*
                }
            }
        }
    };
}

generate_dep_list_impl!();
generate_dep_list_impl!(A);
generate_dep_list_impl!(A, B);
generate_dep_list_impl!(A, B, C);
generate_dep_list_impl!(A, B, C, D);
generate_dep_list_impl!(A, B, C, D, E);
generate_dep_list_impl!(A, B, C, D, E, F);
generate_dep_list_impl!(A, B, C, D, E, F, G);
generate_dep_list_impl!(A, B, C, D, E, F, G, H);
generate_dep_list_impl!(A, B, C, D, E, F, G, H, I);
generate_dep_list_impl!(A, B, C, D, E, F, G, H, I, J);

#[cfg(test)]
mod tests {
    #[allow(dead_code)]
    mod dep_lists_signatures_compile {
        use crate::DependencyList;
        fn a() -> impl DependencyList {
            (1,)
        }
        fn b() -> impl DependencyList {
            (1, "", true)
        }
        fn c() -> impl DependencyList {
            // () implicitly returned
        }
    }
}
