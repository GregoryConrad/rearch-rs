use std::{any::TypeId, sync::Arc};

use rearch::{CData, Capsule, CapsuleHandle, CapsuleKey, Container};
use rearch_effects::{self as effects, Cloned};

/// The default string [`Capsule`].
fn default_string_capsule(_: CapsuleHandle) -> String {
    String::new()
}

/// Another string [`Capsule`].
fn foobar_string_capsule(_: CapsuleHandle) -> String {
    "foobar".to_owned()
}

/// A [`Capsule`] manager that allows us to override the current capsule it is providing.
/// Can be helpful for implementation switching.
fn string_capsule_manager(
    CapsuleHandle { register, .. }: CapsuleHandle,
) -> (
    DynCapsuleHolder<String>,
    impl CData + Fn(DynCapsuleHolder<String>),
) {
    register.register(effects::state::<Cloned<_>>(DynCapsuleHolder::new(
        default_string_capsule,
    )))
}

/// Provides the value of the current string [`Capsule`],
/// as specified by [`string_capsule_manager`].
fn string_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
    let curr_capsule = get.as_ref(string_capsule_manager).0.clone();
    get.as_ref(curr_capsule).clone()
}

/// Allows you to easily set the current string [`Capsule`].
fn set_string_capsule_action(
    CapsuleHandle { mut get, .. }: CapsuleHandle,
) -> impl CData + Fn(DynCapsuleHolder<String>) {
    get.as_ref(string_capsule_manager).1.clone()
}

fn main() {
    let container = Container::new();
    println!("default: {}", container.read(string_capsule));
    container.read(set_string_capsule_action)(DynCapsuleHolder::new(foobar_string_capsule));
    println!("foobar: {}", container.read(string_capsule));
    container.read(set_string_capsule_action)(DynCapsuleHolder::new(default_string_capsule));
    println!("default: {}", container.read(string_capsule));
}

/// A [`Capsule`] that supports dynamic dispatch.
trait DynCapsule {
    type Data;
    fn dyn_build(&self, handle: CapsuleHandle) -> Self::Data;

    // You can also get the eq() and key() trait methods to work with some additional tricks,
    // but they are not currently implemented in the interest of my time.
}

/// Allows us to treat all [`Capsule`]s as [`DynCapsule`]s.
impl<Data, C> DynCapsule for C
where
    C: Capsule<Data = Data>,
{
    type Data = Data;
    fn dyn_build(&self, handle: CapsuleHandle) -> Self::Data {
        self.build(handle)
    }
}

/// Wrapper around [`DynCapsule`]s that allows us to use them as [`Capsule`]s.
struct DynCapsuleHolder<Data> {
    dyn_capsule: Arc<dyn DynCapsule<Data = Data> + Send + Sync>,
    capsule_type_id: TypeId,
}
impl<Data> Clone for DynCapsuleHolder<Data> {
    fn clone(&self) -> Self {
        Self {
            dyn_capsule: Arc::clone(&self.dyn_capsule),
            capsule_type_id: self.capsule_type_id,
        }
    }
}
impl<Data> Capsule for DynCapsuleHolder<Data>
where
    Data: Send + Sync + 'static,
{
    type Data = Data;

    fn build(&self, handle: CapsuleHandle) -> Self::Data {
        self.dyn_capsule.dyn_build(handle)
    }

    fn eq(_old: &Self::Data, _new: &Self::Data) -> bool {
        false
    }

    fn key(&self) -> impl CapsuleKey {
        self.capsule_type_id
    }
}

impl<Data> DynCapsuleHolder<Data> {
    fn new<C: Capsule<Data = Data> + Sync>(capsule: C) -> Self {
        Self {
            dyn_capsule: Arc::new(capsule),
            capsule_type_id: TypeId::of::<C>(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rearch::Container;

    use crate::{
        default_string_capsule, foobar_string_capsule, set_string_capsule_action, string_capsule,
        DynCapsuleHolder,
    };

    #[test]
    fn dynamic_dispatch_capsules_correctly_update() {
        let container = Container::new();
        assert_eq!(container.read(string_capsule), "");
        container.read(set_string_capsule_action)(DynCapsuleHolder::new(foobar_string_capsule));
        assert_eq!(container.read(string_capsule), "foobar");
        container.read(set_string_capsule_action)(DynCapsuleHolder::new(default_string_capsule));
        assert_eq!(container.read(string_capsule), "");
    }
}
