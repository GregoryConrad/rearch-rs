use dyn_clone::DynClone;
use std::{any::Any, cell::OnceCell};

use crate::{SideEffect, EFFECT_FAILED_CAST_MSG};

pub trait SideEffectRebuilder:
    Fn(Box<dyn FnOnce(&mut Box<dyn Any + Send>)>) + Send + Sync + DynClone + 'static
{
}
impl<F> SideEffectRebuilder for F where
    F: Fn(Box<dyn FnOnce(&mut Box<dyn Any + Send>)>) + Send + Sync + Clone + 'static
{
}
dyn_clone::clone_trait_object!(SideEffectRebuilder);

const PREVIOUS_INIT_FAILED_MSG: &str = "Side effect should've been initialized above";

/// Registers the given side effect and returns its build api.
/// You can only call register once on purpose (it consumes self);
/// to register multiple side effects, simply pass them in together!
/// If you have a super pure capsule that you wish to make not super pure,
/// simply call `register()` with no arguments.
pub struct SideEffectRegistrar<'a> {
    side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
    rebuilder: Box<dyn SideEffectRebuilder>,
}

impl<'a> SideEffectRegistrar<'a> {
    /// Creates a new `SideEffectRegistrar`.
    ///
    /// This is public only to enable easier mocking in your code,
    /// or for other libraries looking to integrate deeply with rearch;
    /// do not use this method in other contexts.
    pub fn new(
        side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
        rebuilder: Box<dyn SideEffectRebuilder>,
    ) -> Self {
        Self {
            side_effect,
            rebuilder,
        }
    }

    /// Registers the given side effect.
    pub fn register<S: SideEffect<'a>>(self, effect: S) -> S::Api {
        effect.build(self)
    }

    /// The basic building block for all side effects.
    pub(crate) fn raw<T>(
        self,
        initial: T,
    ) -> (
        &'a mut T,
        impl Fn(Box<dyn FnOnce(&mut T)>) + Clone + Send + Sync + 'static,
    )
    where
        T: Send + 'static,
    {
        self.side_effect.get_or_init(|| Box::new(initial));
        let data = self
            .side_effect
            .get_mut()
            .expect(PREVIOUS_INIT_FAILED_MSG)
            .downcast_mut::<T>()
            .expect(EFFECT_FAILED_CAST_MSG);
        let rebuild = move |mutation: Box<dyn FnOnce(&mut T)>| {
            (self.rebuilder)(Box::new(|data| {
                let data = data.downcast_mut::<T>().expect(EFFECT_FAILED_CAST_MSG);
                mutation(data);
            }));
        };
        (data, rebuild)
    }
}

// One arg register needs its own impl because tuples with one effect don't impl SideEffect
#[cfg(feature = "better-api")]
impl<'a, S: SideEffect<'a>> FnOnce<(S,)> for SideEffectRegistrar<'a> {
    type Output = S::Api;
    extern "rust-call" fn call_once(self, (effect,): (S,)) -> Self::Output {
        self.register(effect)
    }
}
macro_rules! generate_side_effect_registrar_fn_impl {
    ($($types:ident),*) => {
        #[allow(unused_parens, non_snake_case)]
        #[cfg(feature = "better-api")]
        impl<'a, $($types: SideEffect<'a>),*> FnOnce<($($types,)*)> for SideEffectRegistrar<'a> {
            type Output = ($($types::Api),*);
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
