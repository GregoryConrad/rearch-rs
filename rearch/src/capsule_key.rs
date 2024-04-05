use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::Capsule;

/// Represents a static or dynamic capsule key. See [`Capsule::key`].
pub trait CapsuleKey: Hash + Eq + Debug + Send + Sync + 'static {}
impl<T: Hash + Eq + Debug + Send + Sync + 'static> CapsuleKey for T {}

trait DynCapsuleKey: Debug + Send + Sync + 'static {
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapsuleId {
    // NOTE: we need to have a copy of the capsule's type to include in the Hash + Eq
    // so that if two capsules of different types have the same key,
    // they won't be kept under the same entry in the map.
    capsule_type: TypeId,
    // NOTE: capsule_key is Arc<Box<_>> instead of just Arc<_> because of this:
    // https://github.com/rust-lang/rust/issues/78808#issuecomment-1664012270
    // Hand-rolling a PartialEq + Hash sucks and I'd probably screw it up.
    capsule_key: Arc<Box<dyn DynCapsuleKey>>,
}

pub trait CreateCapsuleId {
    fn id(&self) -> CapsuleId;
}
impl<C: Capsule> CreateCapsuleId for C {
    fn id(&self) -> CapsuleId {
        CapsuleId {
            capsule_type: TypeId::of::<C>(),
            capsule_key: Arc::new(Box::new(self.key())),
        }
    }
}
