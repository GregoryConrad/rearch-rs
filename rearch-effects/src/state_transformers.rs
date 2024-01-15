use crate::StateTransformer;

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

#[cfg(feature = "lazy-state-transformers")]
pub use lazy_transformers::*;
#[cfg(feature = "lazy-state-transformers")]
mod lazy_transformers {
    use crate::StateTransformer;
    use once_cell::unsync::Lazy;

    pub struct LazyRef<T, F = fn() -> T>(Lazy<T, F>);
    impl<T: Send + 'static, F: Send + 'static + Fn() -> T> StateTransformer for LazyRef<T, F> {
        type Input = F;
        fn from_input(input: Self::Input) -> Self {
            Self(Lazy::new(input))
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

    pub struct LazyMutRef<T, F = fn() -> T>(Lazy<T, F>);
    impl<T: Send + 'static, F: Send + 'static + Fn() -> T> StateTransformer for LazyMutRef<T, F> {
        type Input = F;
        fn from_input(input: Self::Input) -> Self {
            Self(Lazy::new(input))
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

    pub struct LazyCloned<T, F = fn() -> T>(Lazy<T, F>);
    impl<T: Clone + Send + 'static, F: Send + 'static + Fn() -> T> StateTransformer
        for LazyCloned<T, F>
    {
        type Input = F;
        fn from_input(input: Self::Input) -> Self {
            Self(Lazy::new(input))
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
}
