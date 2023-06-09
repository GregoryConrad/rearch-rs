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

    fn rebuilder(&mut self) -> Arc<Box<dyn Fn() + Sync + Send>> {
        self.register_side_effect(|api| api.rebuilder())
    }

    fn rebuildless_state_queue<T: Sync + Send + 'static>(
        &mut self,
        default: T,
    ) -> (Arc<T>, Box<dyn Fn(T) + Sync + Send>) {
        let queue = self.callonce(|| {
            let state_queue = Mutex::new(VecDeque::new());
            state_queue
                .lock()
                .expect("Queue mutex shouldn't fail to lock")
                .push_back(Arc::new(default));
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

        (curr_state, Box::new(set_state))
    }

    fn state<T: Sync + Send + 'static>(
        &mut self,
        default: T,
    ) -> (Arc<T>, Box<dyn Fn(T) + Sync + Send>) {
        let rebuild = self.rebuilder();
        let (state, set_state) = self.rebuildless_state_queue(default);
        (
            state,
            Box::new(move |new_state| {
                set_state(new_state);
                rebuild();
            }),
        )
    }

    // TODO do we need to have the last thing be called on capsule disposal?
    fn effect<DL: DependencyList>(
        &mut self,
        effect: impl FnOnce() -> Box<dyn FnOnce() + Sync + Send>,
        dependencies: DL,
    ) {
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

    #[cfg(feature = "tokio-future-side-effect")]
    fn future<R: Sync + Send + 'static>(
        &mut self,
        future: impl std::future::Future<Output = R> + Send + 'static,
        dependencies: impl DependencyList,
    ) -> AsyncValue<R> {
        let (state, set_state) = self.state(AsyncValue::AsyncLoading(None));
        self.effect(
            || {
                let curr_data = state.data();
                let handle = tokio::task::spawn(async move {
                    set_state(AsyncValue::AsyncLoading(curr_data));
                    let data = future.await;
                    set_state(AsyncValue::AsyncData(Arc::new(data)));
                });
                Box::new(move || {
                    handle.abort();
                })
            },
            dependencies,
        );
        (*state).clone()
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

pub enum AsyncValue<T> {
    AsyncLoading(Option<Arc<T>>),
    AsyncData(Arc<T>),
}

impl<T> AsyncValue<T> {
    fn data(&self) -> Option<Arc<T>> {
        match self {
            AsyncValue::AsyncLoading(previous) => previous.clone(),
            AsyncValue::AsyncData(data) => Some(data.clone()),
        }
    }
}

impl<T> Clone for AsyncValue<T> {
    fn clone(&self) -> Self {
        match self {
            AsyncValue::AsyncLoading(previous_data) => {
                AsyncValue::AsyncLoading(previous_data.clone())
            }
            AsyncValue::AsyncData(data) => AsyncValue::AsyncData(data.clone()),
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
