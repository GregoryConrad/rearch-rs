use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::Capsule;

/// Represents a key for a capsule.
/// The [`Default`] impl is for static capsules and the [`From`] impl is for dynamic capsules.
///
/// You'll only ever need to use this directly if you are making dynamic (runtime) capsules.
/// Most applications are just fine with static (function) capsules.
/// If you are making an incremental computation focused application,
/// then you may need dynamic capsules and the [`From`] impl.
#[derive(Default)]
pub struct CapsuleKey(CapsuleKeyType);
impl<T: Hash + Eq + Debug + Send + Sync + 'static> From<T> for CapsuleKey {
    fn from(key: T) -> Self {
        Self(CapsuleKeyType::Dynamic(Arc::new(key)))
    }
}

// PartialEq fails to derive because of the Box<dyn Trait>, see here for the below workaround:
// https://github.com/rust-lang/rust/issues/78808#issuecomment-1664416547
#[derive(Debug, Default, PartialEq, Eq, Hash)]
enum CapsuleKeyType<DynDynamicCapsuleKey: ?Sized = dyn DynamicCapsuleKey> {
    /// A static capsule that is identified by its [`TypeId`].
    #[default]
    Static,
    /// A dynamic capsule, whose key is some hash-able data.
    Dynamic(Arc<DynDynamicCapsuleKey>),
}
// NOTE: we need a manual clone impl since the PartialEq derive workaround above
// messes up the Clone derive.
impl Clone for CapsuleKeyType {
    fn clone(&self) -> Self {
        match self {
            CapsuleKeyType::Static => CapsuleKeyType::Static,
            CapsuleKeyType::Dynamic(arc) => CapsuleKeyType::Dynamic(Arc::clone(arc)),
        }
    }
}

trait DynamicCapsuleKey: Debug + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn dyn_hash(&self, state: &mut dyn Hasher);
    fn dyn_eq(&self, other: &dyn Any) -> bool;
}
impl<T> DynamicCapsuleKey for T
where
    T: Hash + Eq + Debug + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }

    fn dyn_eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<T>()
            .map_or(false, |other| self == other)
    }
}
impl Hash for dyn DynamicCapsuleKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dyn_hash(state);
    }
}
impl PartialEq for dyn DynamicCapsuleKey {
    fn eq(&self, other: &dyn DynamicCapsuleKey) -> bool {
        self.dyn_eq(other.as_any())
    }
}
impl Eq for dyn DynamicCapsuleKey {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapsuleId {
    // We need to have a copy of the capsule's type to include in the Hash + Eq
    // so that if two capsules of different types have the same bytes as their key,
    // they won't be kept under the same entry in the map.
    capsule_type: TypeId,
    capsule_key: CapsuleKeyType,
}

pub trait CreateCapsuleId {
    fn id(&self) -> CapsuleId;
}
impl<C: Capsule> CreateCapsuleId for C {
    fn id(&self) -> CapsuleId {
        CapsuleId {
            capsule_type: TypeId::of::<C>(),
            capsule_key: self.key().0,
        }
    }
}
