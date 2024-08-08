use rearch::{SideEffect, SideEffectRegistrar};
use std::{
    any::Any,
    cell::{Cell, OnceCell},
    sync::Arc,
};

type SideEffectTxn<'f> = Box<dyn 'f + FnOnce()>;
type SideEffectTxnRunner = Arc<dyn Send + Sync + Fn(SideEffectTxn)>;
type SideEffectStateMutation<'f> = Box<dyn 'f + FnOnce(&mut dyn Any)>;

type MultiSideEffectStateMutation<'f> = Box<dyn 'f + FnOnce(&mut [OnceCell<Box<dyn Any + Send>>])>;
type MultiSideEffectStateMutationRunner = Arc<dyn Send + Sync + Fn(MultiSideEffectStateMutation)>;

/// Allows you to register multiple side effects _sequentially_,
/// unlike the standard [`SideEffectRegistrar`].
///
/// Instead of having to register all of your effects in one line,
/// you can instead register them throughout the function, as they are needed, for convenience.
///
/// Although more convenient, [`multi`] has some implications:
/// - You must manually pass in the number of side effects registered via the const generic
/// - There is some (slight) added overhead over the traditional [`SideEffectRegistrar::register`]
pub fn multi<const LENGTH: usize>(
) -> impl for<'a> SideEffect<Api<'a> = MultiSideEffectRegistrar<'a>> {
    MultiEffectLifetimeFixer(multi_impl::<LENGTH>)
}

fn multi_impl<const LENGTH: usize>(register: SideEffectRegistrar) -> MultiSideEffectRegistrar {
    let default_array: [OnceCell<Box<dyn Any + Send>>; LENGTH] =
        std::array::from_fn(|_| OnceCell::new());
    let (curr_slice, mutation_runner, run_txn) = register.raw(default_array);
    let multi_mutation_runner = Arc::new(move |mutation: MultiSideEffectStateMutation| {
        mutation_runner(Box::new(move |data| mutation(data)));
    });
    MultiSideEffectRegistrar {
        curr_index: Cell::new(0),
        curr_slice: Cell::new(curr_slice),
        multi_mutation_runner,
        run_txn,
    }
}

/// Allows you to register multiple side effects _sequentially_,
/// unlike the standard [`SideEffectRegistrar`].
/// Provided by [`multi`].
#[expect(
    clippy::module_name_repetitions,
    reason = "https://github.com/rust-lang/rust-clippy/issues/8524"
)]
pub struct MultiSideEffectRegistrar<'a> {
    // NOTE: the Cells are needed in order to support register(&self) (versus &mut self)
    curr_index: Cell<usize>,
    curr_slice: Cell<&'a mut [OnceCell<Box<dyn Any + Send>>]>,
    multi_mutation_runner: MultiSideEffectStateMutationRunner,
    run_txn: SideEffectTxnRunner,
}

impl<'a> MultiSideEffectRegistrar<'a> {
    /// Registers the given [`SideEffect`], similar to [`SideEffectRegistrar::register`].
    ///
    /// # Panics
    /// Panics when the supplied length to [`multi`] is exceeded
    /// by registering too many side effects.
    pub fn register<S: SideEffect>(&'a self, effect: S) -> S::Api<'a> {
        let (curr_data, rest_slice) =
            self.curr_slice.take().split_first_mut().unwrap_or_else(|| {
                panic!(
                    "multi was not given a long enough length; it should be at least {}",
                    self.curr_index.get() + 1
                );
            });

        let mutation_runner = {
            let curr_index = self.curr_index.get();
            let multi_mutation_runner = Arc::clone(&self.multi_mutation_runner);
            Arc::new(move |mutation: SideEffectStateMutation| {
                multi_mutation_runner(Box::new(|multi_data_slice| {
                    let data = &mut **multi_data_slice[curr_index]
                        .get_mut()
                        .expect("To trigger rebuild, side effect must've been registered");
                    mutation(data);
                }));
            })
        };

        self.curr_index.set(self.curr_index.get() + 1);
        self.curr_slice.replace(rest_slice);

        SideEffectRegistrar::new(curr_data, mutation_runner, Arc::clone(&self.run_txn))
            .register(effect)
    }
}

// Stupid workaround for a stupid bug; see effect_lifetime_fixers.rs for more info.
struct MultiEffectLifetimeFixer<F>(F);
impl<F> SideEffect for MultiEffectLifetimeFixer<F>
where
    F: FnOnce(SideEffectRegistrar) -> MultiSideEffectRegistrar,
{
    type Api<'a> = MultiSideEffectRegistrar<'a>;
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use rearch::{CapsuleHandle, Container};

    #[test]
    #[should_panic(expected = "multi was not given a long enough length; it should be at least 1")]
    fn multi_register_undersized() {
        fn capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> bool {
            let register = register.register(multi::<0>());
            register.register(is_first_build())
        }

        Container::new().read(capsule);
    }

    #[test]
    fn multi_register_right_size() {
        fn capsule(CapsuleHandle { register, .. }: CapsuleHandle) -> bool {
            let register = register.register(multi::<1>());
            register.register(is_first_build())
        }

        assert!(Container::new().read(capsule));
    }

    #[test]
    fn multi_register_oversized() {
        fn capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (u32, u32, impl CData + Fn(u32)) {
            let register = register.register(multi::<16>());
            let (x, set_x) = register.register(state::<Cloned<_>>(0));
            let num_builds = register.register(value::<MutRef<_>>(0));
            *num_builds += 1;
            (*num_builds, x, set_x)
        }

        let container = Container::new();
        let (builds, x, set_x) = container.read(capsule);
        assert_eq!(builds, 1);
        assert_eq!(x, 0);
        set_x(123);
        let (builds, x, _) = container.read(capsule);
        assert_eq!(builds, 2);
        assert_eq!(x, 123);
    }
}
