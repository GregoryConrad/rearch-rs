use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub trait BuiltinSideEffects {
    fn register_side_effect<R: Sync + Send + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut dyn crate::SideEffectHandleApi) -> R,
    ) -> Arc<R>;

    fn callonce<R: Sync + Send + 'static>(&mut self, callback: impl FnOnce() -> R) -> Arc<R> {
        self.register_side_effect(|_| callback())
    }

    fn rebuilder<Mutation: FnOnce()>(&mut self) -> impl Fn(Mutation) + Send + Sync + 'static {
        let rebuild = self.register_side_effect(|api| api.rebuilder());
        move |to_run| {
            // TODO the rebuild function should probably take a closure for something to run
            //  at start of build? This will fix possible consistency issues due to concurrency
            to_run();
            rebuild();
        }
    }

    fn internal_state<T: Send + 'static>(
        &mut self,
        initial_state: T,
    ) -> impl Fn(Box<dyn FnMut(std::sync::MutexGuard<T>)>) {
        let state = self.callonce(|| Mutex::new(initial_state));
        move |mut to_run| to_run(state.lock().expect("Mutex shouldn't fail to lock"))
    }

    fn rebuildless_state_queue<T: Sync + Send + 'static>(
        &mut self,
        create_initial: impl FnOnce() -> T,
    ) -> (Arc<T>, impl Fn(T) + Sync + Send + 'static) {
        let queue = self.callonce(|| {
            let state_queue = Mutex::new(VecDeque::new());
            state_queue
                .lock()
                .expect("Queue mutex shouldn't fail to lock")
                .push_back(Arc::new(create_initial()));
            state_queue
        });

        let curr_state = {
            let mut queue = queue.lock().expect("Queue mutex shouldn't fail to lock");

            if queue.len() > 1 {
                queue.pop_front();
            }

            queue.front().expect("Queue should not be empty").clone()
        };

        let set_state = move |new_state| {
            queue
                .lock()
                .expect("Queue mutex shouldn't fail to lock")
                .push_back(Arc::new(new_state));
        };

        (curr_state, set_state)
    }

    fn state_from_fn<T: Sync + Send + 'static>(
        &mut self,
        create_initial: impl FnOnce() -> T,
    ) -> (Arc<T>, impl Fn(T) + Sync + Send + 'static) {
        let rebuild = self.rebuilder();
        let (state, set_state) = self.rebuildless_state_queue(create_initial);

        let set_state = {
            let set_state = Arc::new(set_state);
            move |new_state| {
                let set_state = set_state.clone();
                rebuild(move || set_state(new_state))
            }
        };

        (state, set_state)
    }

    fn state_from_default<T: Send + Sync + Default + 'static>(
        &mut self,
    ) -> (Arc<T>, impl Fn(T) + Sync + Send + 'static) {
        self.state_from_fn(T::default)
    }

    fn state<T: Sync + Send + 'static>(
        &mut self,
        initial: T,
    ) -> (Arc<T>, impl Fn(T) + Sync + Send + 'static) {
        self.state_from_fn(|| initial)
    }

    // TODO should we call the last disposal when the capsule itself is disposed?
    fn effect<DL, OnDispose, Effect>(&mut self, effect: Effect, dependencies: DL)
    where
        DL: DependencyList,
        OnDispose: FnOnce() + Sync + Send + 'static,
        Effect: FnOnce() -> OnDispose,
    {
        let state = self.callonce(|| Mutex::new(None::<(DL, _)>));
        let mut state = state.lock().expect("Mutex shouldn't fail to lock");

        match &mut *state {
            Some((curr_deps, on_dispose)) if !curr_deps.eq(&dependencies) => {
                // We need to grab ownership of the old on dispose in order to call it
                // (since it is an FnOnce), so we need std::mem::replace() to swap in the new one
                std::mem::replace(on_dispose, effect())();
                *curr_deps = dependencies;
            }
            None => *state = Some((dependencies, effect())),
            Some(_) => (),
        }
    }

    fn memo<DL: DependencyList, R: Sync + Send + 'static>(
        &mut self,
        memo: impl FnOnce() -> R,
        dependencies: DL,
    ) -> Arc<R> {
        let state = self.callonce(|| Mutex::new(None::<(DL, _)>));
        let mut state = state.lock().expect("Mutex shouldn't fail to lock");

        let deps_changed = state
            .as_ref()
            .map(|(curr_deps, _)| !curr_deps.eq(&dependencies))
            .unwrap_or(true);

        if deps_changed {
            *state = Some((dependencies, Arc::new(memo())));
        }

        state
            .as_ref()
            .expect("State should've just been initialized if not already")
            .1
            .clone()
    }

    #[cfg(feature = "tokio-side-effects")]
    fn future<R: Sync + Send + 'static>(
        &mut self,
        future: impl std::future::Future<Output = R> + Send + 'static,
        dependencies: impl DependencyList,
    ) -> AsyncState<R> {
        let rebuild = self.rebuilder();
        let mutex = self.callonce(|| Mutex::new(AsyncState::Loading(None)));

        self.effect(
            || {
                {
                    let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");
                    let curr_data = state.data();
                    *state = AsyncState::Loading(curr_data);
                }

                let mutex = mutex.clone();
                let handle = tokio::task::spawn(async move {
                    let data = future.await;
                    rebuild(move || {
                        let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");
                        *state = AsyncState::Complete(Arc::new(data));
                    });
                });

                move || handle.abort()
            },
            dependencies,
        );

        let state = mutex.lock().expect("Mutex shouldn't fail to lock");
        state.clone()
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
    fn sync_persist<T, R: Sync + Send + 'static>(
        &mut self,
        read: impl FnOnce() -> R,
        mut write: impl FnMut(T) -> R,
    ) -> (Arc<R>, impl FnMut(T)) {
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
    ) -> (AsyncState<R>, impl FnMut(T))
    where
        T: Send + 'static,
        R: Sync + Send + 'static,
        Reader: FnOnce() -> ReadFuture + Send + 'static,
        Writer: Fn(T) -> WriteFuture + Send + Sync + 'static,
        ReadFuture: std::future::Future<Output = R> + Send + 'static,
        WriteFuture: std::future::Future<Output = R> + Send + Sync + 'static,
    {
        let read_rebuild = self.rebuilder();
        let write_rebuild = self.rebuilder();
        let mutex = self.callonce(|| {
            Mutex::new((
                AsyncState::Loading(None),
                None::<tokio::task::JoinHandle<()>>,
            ))
        });

        self.callonce(|| {
            let rebuild = read_rebuild;
            let mutex = mutex.clone();
            tokio::task::spawn(async move {
                let initial_data = read().await;

                // If the existing state does not yet have any data, let's stitch some in
                let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");
                if state.0.data().is_none() {
                    state.0 = AsyncState::Loading(Some(Arc::new(initial_data)));
                    drop(state);

                    // We don't do the above state update in the rebuild since we don't care
                    // if we are given our own build for this read state, as any write state
                    // that would be set after is newer/more relevant than this old read data
                    rebuild(|| {});
                }
            })
        });

        let curr_state = mutex
            .lock()
            .expect("Mutex shouldn't fail to lock")
            .0
            .clone();

        let write = Arc::new(write);
        let rebuild = Arc::new(write_rebuild);
        let persist = move |new_data| {
            let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");

            if let Some(old_handle) = &state.1 {
                old_handle.abort();
            }

            let mutex = mutex.clone();
            let write = write.clone();
            let rebuild = rebuild.clone();
            state.0 = AsyncState::Loading(state.0.data());
            state.1 = Some(tokio::task::spawn(async move {
                let result = write(new_data).await;
                rebuild(move || {
                    let mut state = mutex.lock().expect("Mutex shouldn't fail to lock");
                    *state = (AsyncState::Complete(Arc::new(result)), None);
                });
            }));
        };

        (curr_state, persist)
    }
}

impl<Handle: crate::SideEffectHandle> BuiltinSideEffects for Handle {
    fn register_side_effect<R: Sync + Send + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut dyn crate::SideEffectHandleApi) -> R,
    ) -> Arc<R> {
        crate::SideEffectHandle::register_side_effect(self, side_effect)
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
