use std::any::TypeId;

use crate::{Capsule, ContainerWriteTxn};

/// Allows you to read the current data of capsules based on the given state of the container txn.
pub struct CapsuleReader<'scope, 'total> {
    id: TypeId,
    txn: &'scope mut ContainerWriteTxn<'total>,
    // TODO mock utility, like MockCapsuleReaderBuilder::new().set(capsule, value).set(...).build()
    // #[cfg(feature = "capsule-reader-mock")]
    // mock: Option<CapsuleMocks>,
}

impl<'scope, 'total> CapsuleReader<'scope, 'total> {
    pub(crate) fn new(id: TypeId, txn: &'scope mut ContainerWriteTxn<'total>) -> Self {
        Self { id, txn }
    }

    /// Reads the current data of the supplied capsule, initializing it if needed.
    /// Internally forms a dependency graph amongst capsules, so feel free to conditionally invoke
    /// this function in case you only conditionally need a capsule's data.
    ///
    /// # Panics
    /// Panics when a capsule attempts to read itself in its first build.
    pub fn read<C: Capsule>(&mut self, capsule: C) -> C::Data {
        let (this, other) = (self.id, TypeId::of::<C>());
        if this == other {
            return self.txn.try_read(&capsule).unwrap_or_else(|| {
                let capsule_name = std::any::type_name::<C>();
                panic!(
                    "Capsule {capsule_name} tried to read itself on its first build! {} {} {}",
                    "This is disallowed since the capsule doesn't have any data to read yet.",
                    "To avoid this issue, wrap the `read({capsule_name})` call in an if statement",
                    "with the `IsFirstBuildEffect`."
                );
            });
        }

        // Get the value (and make sure the other manager is initialized!)
        let data = self.txn.read_or_init(capsule);

        // Take care of some dependency housekeeping
        self.txn.node_or_panic(other).dependents.insert(this);
        self.txn.node_or_panic(this).dependencies.insert(other);

        data
    }
}

impl<A: Capsule> FnOnce<(A,)> for CapsuleReader<'_, '_> {
    type Output = A::Data;
    extern "rust-call" fn call_once(mut self, args: (A,)) -> Self::Output {
        self.call_mut(args)
    }
}

impl<A: Capsule> FnMut<(A,)> for CapsuleReader<'_, '_> {
    extern "rust-call" fn call_mut(&mut self, args: (A,)) -> Self::Output {
        self.read(args.0)
    }
}
