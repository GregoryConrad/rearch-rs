use std::marker::PhantomData;

use crate::StateTransformer;

/// A [`StateTransformer`] that provides a `&T` as a part of the side effect's api.
pub struct Ref<T>(T);
impl<T: Send + 'static> StateTransformer for Ref<T> {
    type Input = T;
    fn from_input(input: Self::Input) -> Self {
        Self(input)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = &'a T;
    fn as_output(&mut self) -> Self::Output<'_> {
        &self.0
    }
}

/// A [`StateTransformer`] that provides a `&mut T` as a part of the side effect's api.
pub struct MutRef<T>(T);
impl<T: Send + 'static> StateTransformer for MutRef<T> {
    type Input = T;
    fn from_input(input: Self::Input) -> Self {
        Self(input)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = &'a mut T;
    fn as_output(&mut self) -> Self::Output<'_> {
        &mut self.0
    }
}

/// A [`StateTransformer`] that provides a `T` where `T: Clone` as a part of the side effect's api.
pub struct Cloned<T>(T);
impl<T: Clone + Send + 'static> StateTransformer for Cloned<T> {
    type Input = T;
    fn from_input(input: Self::Input) -> Self {
        Self(input)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = T;
    fn as_output(&mut self) -> Self::Output<'_> {
        self.0.clone()
    }
}

/// A [`StateTransformer`] that provides a `&T` as a part of the side effect's api,
/// but takes a lazily-evaluated function as input to initialize the side effect state.
pub struct LazyRef<T, F = fn() -> T>(T, PhantomData<F>);
impl<T: Send + 'static, F: 'static + Send + FnOnce() -> T> StateTransformer for LazyRef<T, F> {
    type Input = F;
    fn from_input(input: Self::Input) -> Self {
        Self(input(), PhantomData)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = &'a T;
    fn as_output(&mut self) -> Self::Output<'_> {
        &self.0
    }
}

/// A [`StateTransformer`] that provides a `&mut T` as a part of the side effect's api,
/// but takes a lazily-evaluated function as input to initialize the side effect state.
pub struct LazyMutRef<T, F = fn() -> T>(T, PhantomData<F>);
impl<T: Send + 'static, F: 'static + Send + Fn() -> T> StateTransformer for LazyMutRef<T, F> {
    type Input = F;
    fn from_input(input: Self::Input) -> Self {
        Self(input(), PhantomData)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = &'a mut T;
    fn as_output(&mut self) -> Self::Output<'_> {
        &mut self.0
    }
}

/// A [`StateTransformer`] that provides a `T` where `T: Clone` as a part of the side effect's api,
/// but takes a lazily-evaluated function as input to initialize the side effect state.
pub struct LazyCloned<T, F = fn() -> T>(T, PhantomData<F>);
impl<T: Clone + Send + 'static, F: 'static + Send + Fn() -> T> StateTransformer
    for LazyCloned<T, F>
{
    type Input = F;
    fn from_input(input: Self::Input) -> Self {
        Self(input(), PhantomData)
    }

    type Inner = T;
    fn as_inner(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    type Output<'a> = T;
    fn as_output(&mut self) -> Self::Output<'_> {
        self.0.clone()
    }
}
