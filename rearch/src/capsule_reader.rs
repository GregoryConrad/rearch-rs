use std::{any::Any, collections::HashMap, sync::Arc};

use crate::{Capsule, CapsuleId, ContainerWriteTxn, CreateCapsuleId};

/// Allows you to read the current data of capsules based on the given state of the container txn.
pub struct CapsuleReader<'scope, 'total>(InternalCapsuleReader<'scope, 'total>);
enum InternalCapsuleReader<'scope, 'total> {
    /// For normal [`CapsuleReader`] operation
    Normal {
        id: CapsuleId,
        txn: &'scope mut ContainerWriteTxn<'total>,
    },
    /// To enable easy mocking in testing
    Mock {
        mocks: HashMap<CapsuleId, Arc<dyn Any + Send + Sync>>,
    },
}

impl<'scope, 'total> CapsuleReader<'scope, 'total> {
    pub(crate) const fn new(id: CapsuleId, txn: &'scope mut ContainerWriteTxn<'total>) -> Self {
        Self(InternalCapsuleReader::Normal { id, txn })
    }

    /// Returns a ref to the current data of the supplied capsule, initializing it if needed.
    /// Internally forms a dependency graph amongst capsules, so feel free to conditionally invoke
    /// this function in case you only conditionally need a capsule's data.
    ///
    /// # Panics
    /// Panics when a capsule attempts to read itself in its first build,
    /// or when a mocked [`CapsuleReader`] attempts to read a capsule's data that wasn't mocked.
    pub fn as_ref<C: Capsule>(&mut self, capsule: C) -> &C::Data {
        match &mut self.0 {
            InternalCapsuleReader::Normal { ref id, txn } => {
                let (this, other) = (id, capsule.id());
                if this == &other {
                    return txn.try_read_ref(&capsule).unwrap_or_else(|| {
                        let name = std::any::type_name::<C>();
                        panic!(
                            "{name} ({id:?}) tried to read itself on its first build! {} {} {}",
                            "This is disallowed since the capsule doesn't have data to read yet.",
                            "To avoid this issue, wrap the `get()` call in an if statement",
                            "with the builtin \"is_first_build\" side effect."
                        );
                    });
                }

                txn.ensure_initialized(capsule);
                txn.add_dependency_relationship(&other, this);
                txn.try_read_ref_raw::<C>(&other)
                    .expect("Ensured capsule was initialized above")
            }
            InternalCapsuleReader::Mock { mocks } => {
                let id = capsule.id();
                mocks.get(&id).map_or_else(
                    || {
                        panic!(
                            "Mock CapsuleReader was used to read {} ({id:?}) {}",
                            std::any::type_name::<C>(),
                            "when it was not included in the mock!"
                        );
                    },
                    crate::downcast_capsule_data::<C>,
                )
            }
        }
    }
}

#[cfg(feature = "experimental-api")]
impl<A: Capsule> FnOnce<(A,)> for CapsuleReader<'_, '_>
where
    A::Data: Clone,
{
    type Output = A::Data;
    extern "rust-call" fn call_once(mut self, args: (A,)) -> Self::Output {
        self.call_mut(args)
    }
}

#[cfg(feature = "experimental-api")]
impl<A: Capsule> FnMut<(A,)> for CapsuleReader<'_, '_>
where
    A::Data: Clone,
{
    extern "rust-call" fn call_mut(&mut self, args: (A,)) -> Self::Output {
        self.as_ref(args.0).clone()
    }
}

/// Used to build a mocked [`CapsuleReader`] for use in unit testing capsules.
#[derive(Clone, Default)]
pub struct MockCapsuleReaderBuilder(HashMap<CapsuleId, Arc<dyn Any + Send + Sync>>);

impl MockCapsuleReaderBuilder {
    /// Creates a new [`MockCapsuleReaderBuilder`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mocks the value of the given `capsule` to `data`.
    #[must_use]
    pub fn set<C: Capsule>(mut self, capsule: &C, data: C::Data) -> Self {
        self.0.insert(capsule.id(), Arc::new(data));
        self
    }

    /// Builds the final [`CapsuleReader`] with all of the supplied mocks
    /// from [`MockCapsuleReaderBuilder::set`].
    #[must_use]
    pub fn build(self) -> CapsuleReader<'static, 'static> {
        CapsuleReader(InternalCapsuleReader::Mock { mocks: self.0 })
    }
}

#[cfg(test)]
mod tests {
    use crate::{CapsuleHandle, CapsuleReader, MockCapsuleReaderBuilder};

    fn foo_capsule(_: CapsuleHandle) -> u8 {
        0
    }
    fn bar_capsule(_: CapsuleHandle) -> Box<dyn Send + Sync + Fn() -> u8> {
        Box::new(|| 0)
    }
    fn another_capsule(_: CapsuleHandle) -> u8 {
        0
    }

    fn create_mock_capsule_reader() -> CapsuleReader<'static, 'static> {
        MockCapsuleReaderBuilder::new()
            .set(&foo_capsule, 123)
            .set(&bar_capsule, Box::new(|| 123))
            .build()
    }

    #[test]
    fn mock_capsule_reader_reads_capsules() {
        let mut get = create_mock_capsule_reader();
        assert_eq!(*get.as_ref(foo_capsule), 123);
        assert_eq!(get.as_ref(bar_capsule)(), 123);
        drop(get);
    }

    #[test]
    #[allow(clippy::should_panic_without_expect)] // exact panic string is based on capsule TypeId
    #[should_panic]
    fn mock_capsule_reader_panics_on_unmocked_capsule() {
        create_mock_capsule_reader().as_ref(another_capsule);
    }
}
