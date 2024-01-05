use rearch::{CData, SideEffect, SideEffectRegistrar};

pub fn state<T: Clone + Send + 'static>(
    initial: T,
) -> impl for<'a> SideEffect<Api<'a> = (T, impl CData + Fn(T))> {
    move |register: SideEffectRegistrar| {
        let (state, set_state) = register.register(super::state(initial));
        (state.clone(), set_state)
    }
}

pub fn lazy_state<T, F>(init: F) -> impl for<'a> SideEffect<Api<'a> = (T, impl CData + Fn(T))>
where
    T: Clone + Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    move |register: SideEffectRegistrar| {
        let (state, set_state) = register.register(super::lazy_state(init));
        (state.clone(), set_state)
    }
}

pub fn value<T: Clone + Send + 'static>(value: T) -> impl for<'a> SideEffect<Api<'a> = T> {
    move |register: SideEffectRegistrar| register.register(super::value(value)).clone()
}

pub fn lazy_value<T, F>(init: F) -> impl for<'a> SideEffect<Api<'a> = T>
where
    T: Clone + Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    move |register: SideEffectRegistrar| register.register(super::lazy_value(init)).clone()
}
