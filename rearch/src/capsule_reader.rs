use std::{any::Any, collections::HashMap};

use crate::{Capsule, CapsuleData, ContainerWriteTxn, CreateInternalKey, InternalKey};

/// Allows you to read the current data of capsules based on the given state of the container txn.
pub enum CapsuleReader<'key, 'scope, 'total> {
    // For normal operation
    Normal {
        id: &'key InternalKey,
        txn: &'scope mut ContainerWriteTxn<'total>,
    },
    // To enable easy mocking in testing
    Mock {
        mocks: HashMap<InternalKey, Box<dyn CapsuleData>>,
    },
}

impl<'key, 'scope, 'total> CapsuleReader<'key, 'scope, 'total> {
    pub(crate) fn new(id: &'key InternalKey, txn: &'scope mut ContainerWriteTxn<'total>) -> Self {
        Self::Normal { id, txn }
    }

    /// Reads the current data of the supplied capsule, initializing it if needed.
    /// Internally forms a dependency graph amongst capsules, so feel free to conditionally invoke
    /// this function in case you only conditionally need a capsule's data.
    ///
    /// # Panics
    /// Panics when a capsule attempts to read itself in its first build.
    pub fn get<C: Capsule>(&mut self, capsule: C) -> C::Data {
        match self {
            CapsuleReader::Normal { id, txn } => {
                let (this, other) = (id.clone(), capsule.internal_key());
                if this == other {
                    return txn.try_read(&capsule).unwrap_or_else(|| {
                        let name = std::any::type_name::<C>();
                        panic!(
                            "{name} ({id:?}) tried to read itself on its first build! {} {} {}",
                            "This is disallowed since the capsule doesn't have data to read yet.",
                            "To avoid this issue, wrap the `get()` call in an if statement",
                            "with the builtin \"is_first_build\" side effect."
                        );
                    });
                }

                // Adding dep relationship is after read_or_init to ensure manager is initialized
                let data = txn.read_or_init(capsule);
                txn.add_dependency_relationship(other, this);
                data
            }
            CapsuleReader::Mock { mocks } => {
                let id = capsule.internal_key();
                let any: Box<dyn Any> = mocks
                    .get(&id)
                    .unwrap_or_else(|| {
                        panic!(
                            "Mock CapsuleReader was used to read {} ({id:?}) {}",
                            std::any::type_name::<C>(),
                            "when it was not included in the mock!"
                        );
                    })
                    .clone();
                *any.downcast::<C::Data>()
                    .expect("Types should be properly enforced due to generics")
            }
        }
    }
}

#[cfg(feature = "better-api")]
impl<A: Capsule> FnOnce<(A,)> for CapsuleReader<'_, '_, '_> {
    type Output = A::Data;
    extern "rust-call" fn call_once(mut self, args: (A,)) -> Self::Output {
        self.call_mut(args)
    }
}

#[cfg(feature = "better-api")]
impl<A: Capsule> FnMut<(A,)> for CapsuleReader<'_, '_, '_> {
    extern "rust-call" fn call_mut(&mut self, args: (A,)) -> Self::Output {
        self.get(args.0)
    }
}

#[derive(Clone, Default)]
pub struct MockCapsuleReaderBuilder(HashMap<InternalKey, Box<dyn CapsuleData>>);

impl MockCapsuleReaderBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn set<C: Capsule>(mut self, capsule: &C, data: C::Data) -> Self {
        self.0.insert(capsule.internal_key(), Box::new(data));
        self
    }

    #[must_use]
    pub fn build(self) -> CapsuleReader<'static, 'static, 'static> {
        CapsuleReader::Mock { mocks: self.0 }
    }
}
