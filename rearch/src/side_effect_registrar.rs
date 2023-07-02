use std::{any::Any, cell::OnceCell};

use crate::{SideEffect, SideEffectRebuilder};

/// Registers the given side effect and returns its build api.
/// You can only call register once on purpose (it consumes self);
/// to register multiple side effects, simply pass them in together!
/// If you have a super pure capsule that you wish to make not super pure,
/// simply call `register()` with no arguments.
pub struct SideEffectRegistrar<'a> {
    side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
    rebuild: Box<dyn SideEffectRebuilder<Box<dyn Any + Send>>>,
}

impl<'a> SideEffectRegistrar<'a> {
    /// Creates a new `SideEffectRegistrar`.
    ///
    /// This is public only to enable easier mocking in your code;
    /// do not use this method in a non-test context.
    pub fn new(
        side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
        rebuild: Box<dyn SideEffectRebuilder<Box<dyn Any + Send>>>,
    ) -> Self {
        Self {
            side_effect,
            rebuild,
        }
    }
}

// Empty register() for the no-op side effect
impl FnOnce<()> for SideEffectRegistrar<'_> {
    type Output = ();
    extern "rust-call" fn call_once(self, _: ()) -> Self::Output {
        // Initialize with the no-op side effect
        self.side_effect.get_or_init(|| Box::new(()));

        // Ensure side effect wasn't changed
        assert!(
            self.side_effect
                .get_mut()
                .expect("Side effect should've been initialized above")
                .is::<()>(),
            "You cannot change the side effect(s) passed to register()!"
        );
    }
}

const EFFECT_FAILED_CAST_MSG: &str =
    "The SideEffect registered with SideEffectRegistrar cannot be changed!";

macro_rules! generate_side_effect_registrar_fn_impl {
    ($($types:ident),+) => {
        #[allow(unused_parens, non_snake_case)]
        impl<'a, $($types: SideEffect),*> FnOnce<($($types,)*)> for SideEffectRegistrar<'a> {
            type Output = ($($types::Api<'a>),*);

            extern "rust-call" fn call_once(self, args: ($($types,)*)) -> Self::Output {
                let ($($types,)*) = args;
                self.side_effect.get_or_init(|| Box::new(($($types),*)));
                let effect = self
                    .side_effect
                    .get_mut()
                    .expect("Side effect should've been initialized above")
                    .downcast_mut::<($($types),*)>()
                    .expect(EFFECT_FAILED_CAST_MSG);

                effect.api(Box::new(move |mutation| {
                    (self.rebuild)(Box::new(|effect| {
                        let effect = effect
                            .downcast_mut::<($($types),*)>()
                            .expect(EFFECT_FAILED_CAST_MSG);
                        mutation(effect);
                    }));
                }))
            }
        }
    }
}

generate_side_effect_registrar_fn_impl!(A);
generate_side_effect_registrar_fn_impl!(A, B);
generate_side_effect_registrar_fn_impl!(A, B, C);
generate_side_effect_registrar_fn_impl!(A, B, C, D);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G, H);
