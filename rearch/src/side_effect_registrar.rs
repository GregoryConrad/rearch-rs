use std::{any::Any, cell::OnceCell};

use crate::{
    CData, SideEffect, SideEffectStateMutationRunner, SideEffectTxnRunner, EFFECT_FAILED_CAST_MSG,
};

/// Registers the given side effect and returns its build api.
/// You can only call register once on purpose (it consumes self);
/// to register multiple side effects, simply pass them in together!
pub struct SideEffectRegistrar<'a> {
    side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
    side_effect_state_mutation_runner: SideEffectStateMutationRunner,
    side_effect_txn_runner: SideEffectTxnRunner,
}

impl<'a> SideEffectRegistrar<'a> {
    /// Creates a new `SideEffectRegistrar`.
    ///
    /// This is public only to enable easier mocking in your code,
    /// or for other libraries looking to deeply integrate;
    /// do not use this method in other contexts.
    pub fn new(
        side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
        side_effect_state_mutation_runner: SideEffectStateMutationRunner,
        side_effect_txn_runner: SideEffectTxnRunner,
    ) -> Self {
        Self {
            side_effect,
            side_effect_state_mutation_runner,
            side_effect_txn_runner,
        }
    }

    /// Registers the given side effect.
    pub fn register<S: SideEffect>(self, effect: S) -> S::Api<'a> {
        effect.build(self)
    }
}

impl<'a> SideEffectRegistrar<'a> {
    /// The basic building block for all side effects.
    ///
    /// # Panics
    /// Panics when the supplied type `T` changes between builds.
    /// Ensure T remains the same across builds (e.g., by calling this function unconditionally).
    pub fn raw<T>(
        self,
        initial: T,
    ) -> (
        &'a mut T,
        impl CData + for<'f> Fn(Box<dyn 'f + FnOnce(&mut T)>),
        SideEffectTxnRunner,
    )
    where
        T: Send + 'static,
    {
        self.side_effect.get_or_init(|| Box::new(initial));
        let data = self
            .side_effect
            .get_mut()
            .expect("Side effect should've been initialized in get_or_init above")
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("{}", EFFECT_FAILED_CAST_MSG));

        let mutation_runner = move |mutation: Box<dyn '_ + FnOnce(&mut T)>| {
            (self.side_effect_state_mutation_runner)(Box::new(|data| {
                let data = data
                    .downcast_mut::<T>()
                    .unwrap_or_else(|| panic!("{}", EFFECT_FAILED_CAST_MSG));
                mutation(data);
            }));
        };

        (data, mutation_runner, self.side_effect_txn_runner)
    }
}

// One arg register needs its own impl because tuples with one effect don't impl SideEffect
#[cfg(feature = "experimental-api")]
impl<'a, S: SideEffect> FnOnce<(S,)> for SideEffectRegistrar<'a> {
    type Output = S::Api<'a>;
    extern "rust-call" fn call_once(self, (effect,): (S,)) -> Self::Output {
        self.register(effect)
    }
}
macro_rules! generate_side_effect_registrar_fn_impl {
    ($($types:ident),*) => {
        #[allow(unused_parens, non_snake_case)]
        #[cfg(feature = "experimental-api")]
        impl<'a, $($types: SideEffect),*> FnOnce<($($types,)*)> for SideEffectRegistrar<'a> {
            type Output = ($($types::Api<'a>),*);
            extern "rust-call" fn call_once(self, args: ($($types),*)) -> Self::Output {
                self.register(args)
            }
        }
    }
}
generate_side_effect_registrar_fn_impl!();
generate_side_effect_registrar_fn_impl!(A, B);
generate_side_effect_registrar_fn_impl!(A, B, C);
generate_side_effect_registrar_fn_impl!(A, B, C, D);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G, H);
