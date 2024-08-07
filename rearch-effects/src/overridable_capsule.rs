use std::{any::TypeId, sync::Arc};

use rearch::{Capsule, CapsuleHandle, CapsuleKey, SideEffect, SideEffectRegistrar};

/// Allows you to create a [`Capsule`] that can switch between implementations at runtime.
///
/// This is accomplished via [`OverridableCapsule`], which itself is a [`Capsule`]
/// that provides [`OverridableCapsule::set`] to change its backing capsule
/// (backing capsules must share _the same_ [`Capsule::Data`]).
///
/// Note that there is no free lunch. This side effect has some known limitations:
/// - No [`Capsule::eq`] support (so no runtime-optimizations when capsule data doesn't change)
/// - Overriding capsules must be [`Sync`], since [`Capsule::Data`] itself is [`Sync`]
///   (and the current overriding capsule is stored as [`Capsule::Data`])
/// - Capsules that have `impl Trait` in their [`Capsule::Data`] are not compatible with each other;
///   you must use `dyn Trait` (or an enum) instead
/// - There's some slight overhead due to dynamic dispatch everywhere in the implementation
///   (but is largely minimal in the context of `ReArch` as a whole)
///
/// # Examples
/// ```rust
/// # use rearch::{CapsuleHandle, Container};
/// # use rearch_effects::{overridable_capsule, OverridableCapsule};
/// fn default_string_capsule(_: CapsuleHandle) -> String {
///     String::new()
/// }
///
/// fn foobar_string_capsule(_: CapsuleHandle) -> String {
///     "foobar".to_owned()
/// }
///
/// fn string_overridable_capsule(
///     CapsuleHandle { register, .. }: CapsuleHandle,
/// ) -> OverridableCapsule<String> {
///     register.register(overridable_capsule(default_string_capsule))
/// }
///
/// fn string_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
///     let curr_capsule = get.as_ref(string_overridable_capsule).clone();
///     get.as_ref(curr_capsule).clone()
/// }
///
/// let container = Container::new();
/// assert_eq!(container.read(string_capsule), "");
///
/// container
///     .read(string_overridable_capsule)
///     .set(foobar_string_capsule);
/// assert_eq!(container.read(string_capsule), "foobar");
/// ```
pub fn overridable_capsule<Data, C>(
    default_capsule: C,
) -> impl for<'a> SideEffect<Api<'a> = OverridableCapsule<Data>>
where
    Data: Send + Sync + 'static,
    C: Capsule<Data = Data> + Sync,
{
    move |register: SideEffectRegistrar<'_>| {
        let (curr_capsule, mutate, _) = register.raw(DynCapsuleHolder::new(default_capsule));
        OverridableCapsule {
            capsule_holder: curr_capsule.clone(),
            capsule_setter: Arc::new(move |new_holder| {
                mutate(Box::new(|curr_holder| *curr_holder = new_holder));
            }),
        }
    }
}

/// A [`Capsule`] that enables overriding its implementation via [`OverridableCapsule::set`].
/// See [`overridable_capsule`] for more.
pub struct OverridableCapsule<Data> {
    capsule_holder: DynCapsuleHolder<Data>,
    capsule_setter: Arc<dyn Fn(DynCapsuleHolder<Data>) + Send + Sync>,
}

impl<Data> OverridableCapsule<Data> {
    /// Overrides the [`OverridableCapsule`] to point to the supplied [`Capsule`].
    ///
    /// Note that this function mutates the underlying [`rearch::Container`] (and not `self`),
    /// so you must call [`rearch::Container::read`] again for the latest value.
    ///
    /// As you shouldn't be using an [`OverridableCapsule`] after calling this method
    /// (to prevent using stale data), this method consumes `self` to prevent possible API misuse.
    /// If, for some reason, you _do_ want to use the outdated capsule after calling this method,
    /// call [`OverridableCapsule::clone`] first.
    pub fn set<C>(self, capsule: C)
    where
        C: Capsule<Data = Data> + Sync,
    {
        (self.capsule_setter)(DynCapsuleHolder::new(capsule));
    }
}

impl<Data> Clone for OverridableCapsule<Data> {
    fn clone(&self) -> Self {
        Self {
            capsule_holder: self.capsule_holder.clone(),
            capsule_setter: Arc::clone(&self.capsule_setter),
        }
    }
}

impl<Data> Capsule for OverridableCapsule<Data>
where
    Data: Send + Sync + 'static,
{
    type Data = Data;

    fn build(&self, handle: CapsuleHandle) -> Self::Data {
        self.capsule_holder.build(handle)
    }

    fn eq(old: &Self::Data, new: &Self::Data) -> bool {
        DynCapsuleHolder::eq(old, new)
    }

    fn key(&self) -> impl CapsuleKey {
        self.capsule_holder.key()
    }
}

/// A [`Capsule`] that supports dynamic dispatch (is trait object safe).
trait DynCapsule {
    type Data;
    fn dyn_build(&self, handle: CapsuleHandle) -> Self::Data;
    fn dyn_key(&self) -> Box<dyn DynCapsuleKey>;
}

impl<Data, C> DynCapsule for C
where
    C: Capsule<Data = Data>,
{
    type Data = Data;

    fn dyn_build(&self, handle: CapsuleHandle) -> Self::Data {
        self.build(handle)
    }

    fn dyn_key(&self) -> Box<dyn DynCapsuleKey> {
        Box::new(self.key())
    }
}

/// Wrapper around [`DynCapsule`]s that allows us to use them as [`Capsule`]s.
struct DynCapsuleHolder<Data> {
    dyn_capsule: Arc<dyn DynCapsule<Data = Data> + Send + Sync>,
    capsule_type_id: TypeId,
}

impl<Data> DynCapsuleHolder<Data> {
    fn new<C: Capsule<Data = Data> + Sync>(capsule: C) -> Self {
        Self {
            dyn_capsule: Arc::new(capsule),
            capsule_type_id: TypeId::of::<C>(),
        }
    }
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
        (self.capsule_type_id, self.dyn_capsule.dyn_key())
    }
}

// NOTE: DynCapsuleKey duplicated here from the core ReArch crate
use dyn_capsule_key::DynCapsuleKey;
mod dyn_capsule_key {
    use std::{
        any::Any,
        fmt::Debug,
        hash::{Hash, Hasher},
    };

    pub trait DynCapsuleKey: Debug + Send + Sync + 'static {
        fn as_any(&self) -> &dyn Any;
        fn dyn_hash(&self, state: &mut dyn Hasher);
        fn dyn_eq(&self, other: &dyn DynCapsuleKey) -> bool;
    }
    impl<T> DynCapsuleKey for T
    where
        T: Hash + Eq + Debug + Send + Sync + 'static,
    {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn dyn_hash(&self, mut state: &mut dyn Hasher) {
            self.hash(&mut state);
        }

        fn dyn_eq(&self, other: &dyn DynCapsuleKey) -> bool {
            other
                .as_any()
                .downcast_ref::<T>()
                .map_or(false, |other| self == other)
        }
    }
    impl Hash for dyn DynCapsuleKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.dyn_hash(state);
        }
    }
    impl PartialEq for dyn DynCapsuleKey {
        fn eq(&self, other: &dyn DynCapsuleKey) -> bool {
            self.dyn_eq(other)
        }
    }
    impl Eq for dyn DynCapsuleKey {}
}

#[cfg(test)]
mod tests {
    use rearch::Container;

    use capsules::*;
    mod capsules {
        use rearch::{Capsule, CapsuleHandle, CapsuleKey};

        use crate::{overridable_capsule, OverridableCapsule};

        pub fn default_string_capsule(_: CapsuleHandle) -> String {
            String::new()
        }

        pub fn foobar_string_capsule(_: CapsuleHandle) -> String {
            "foobar".to_owned()
        }

        pub fn string_overridable_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> OverridableCapsule<String> {
            register.register(overridable_capsule(default_string_capsule))
        }

        pub fn string_capsule(CapsuleHandle { mut get, .. }: CapsuleHandle) -> String {
            let curr_capsule = get.as_ref(string_overridable_capsule).clone();
            get.as_ref(curr_capsule).clone()
        }

        pub struct DynamicStringCapsule(pub u8);
        impl Capsule for DynamicStringCapsule {
            type Data = String;

            fn build(&self, _: CapsuleHandle) -> Self::Data {
                format!("{}", self.0)
            }

            fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                old == new
            }

            fn key(&self) -> impl CapsuleKey {
                self.0
            }
        }
    }

    #[test]
    fn overridable_capsule_correctly_updates() {
        let container = Container::new();
        assert_eq!(container.read(string_capsule), "");

        container
            .read(string_overridable_capsule)
            .set(foobar_string_capsule);
        assert_eq!(container.read(string_capsule), "foobar");

        container
            .read(string_overridable_capsule)
            .set(default_string_capsule);
        assert_eq!(container.read(string_capsule), "");
    }

    #[test]
    fn overridable_dynamic_capsules_correctly_updates() {
        let container = Container::new();
        assert_eq!(container.read(string_capsule), "");

        container
            .read(string_overridable_capsule)
            .set(DynamicStringCapsule(0));
        assert_eq!(container.read(string_capsule), "0");

        container
            .read(string_overridable_capsule)
            .set(DynamicStringCapsule(123));
        assert_eq!(container.read(string_capsule), "123");

        container
            .read(string_overridable_capsule)
            .set(DynamicStringCapsule(0));
        assert_eq!(container.read(string_capsule), "0");

        container
            .read(string_overridable_capsule)
            .set(foobar_string_capsule);
        assert_eq!(container.read(string_capsule), "foobar");
    }
}
