use crate::StateTransformer;
use rearch::{SideEffect, SideEffectRegistrar};

// These workarounds were derived from:
// https://github.com/GregoryConrad/rearch-rs/issues/3#issuecomment-1872869363
// And are needed because of:
// https://github.com/rust-lang/rust/issues/111662
// A big thank you to https://github.com/0e4ef622 for all of their help here!

pub struct EffectLifetimeFixer0<F, ST>(F, std::marker::PhantomData<ST>);
impl<F, ST> SideEffect for EffectLifetimeFixer0<F, ST>
where
    F: FnOnce(SideEffectRegistrar) -> ST::Output<'_>,
    ST: StateTransformer,
{
    type Api<'a> = ST::Output<'a>;
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}
impl<F, ST> EffectLifetimeFixer0<F, ST> {
    pub(super) const fn new(f: F) -> Self
    where
        F: FnOnce(SideEffectRegistrar) -> ST::Output<'_>,
        ST: StateTransformer,
    {
        Self(f, std::marker::PhantomData)
    }
}

pub struct EffectLifetimeFixer1<F, ST>(F, std::marker::PhantomData<ST>);
impl<F, ST, R1> SideEffect for EffectLifetimeFixer1<F, ST>
where
    F: FnOnce(SideEffectRegistrar) -> (ST::Output<'_>, R1),
    ST: StateTransformer,
{
    type Api<'a> = (ST::Output<'a>, R1);
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}
impl<F, ST> EffectLifetimeFixer1<F, ST> {
    pub(super) const fn new<R1>(f: F) -> Self
    where
        F: FnOnce(SideEffectRegistrar) -> (ST::Output<'_>, R1),
        ST: StateTransformer,
    {
        Self(f, std::marker::PhantomData)
    }
}

pub struct EffectLifetimeFixer2<F, ST>(F, std::marker::PhantomData<ST>);
impl<F, ST, R1, R2> SideEffect for EffectLifetimeFixer2<F, ST>
where
    F: FnOnce(SideEffectRegistrar) -> (ST::Output<'_>, R1, R2),
    ST: StateTransformer,
{
    type Api<'a> = (ST::Output<'a>, R1, R2);
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self.0(registrar)
    }
}
impl<F, ST> EffectLifetimeFixer2<F, ST> {
    pub(super) const fn new<R1, R2>(f: F) -> Self
    where
        F: FnOnce(SideEffectRegistrar) -> (ST::Output<'_>, R1, R2),
        ST: StateTransformer,
    {
        Self(f, std::marker::PhantomData)
    }
}
