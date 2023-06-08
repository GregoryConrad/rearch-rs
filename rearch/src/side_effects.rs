use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::SideEffectHandle;

pub trait BuiltinSideEffects {
    fn callonce<R: Sync + Send + 'static>(&mut self, callback: impl FnOnce() -> R) -> Arc<R>;
    fn rebuilder(&mut self) -> Arc<Box<dyn Fn() + Sync + Send>>;
    fn rebuildless_state_queue<T: Sync + Send + 'static>(
        &mut self,
        default: T,
    ) -> (Arc<T>, Box<dyn Fn(T) + Sync + Send>);
    fn state<T: Sync + Send + 'static>(
        &mut self,
        default: T,
    ) -> (Arc<T>, Box<dyn Fn(T) + Sync + Send>);
}

impl<Handle: SideEffectHandle> BuiltinSideEffects for Handle {
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

    // TODO memo and effect

    // TODO feature that just does a side effect handle fn for futures
    // and add signature to the builtins
    #[cfg(feature = "tokio-future-side-effect")]
    fn future(&mut self) {}
}
