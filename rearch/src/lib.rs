#![feature(return_position_impl_trait_in_trait)]
use concread::hashmap::{HashMapReadTxn, HashMapWriteTxn};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashSet,
    marker::PhantomData,
    sync::{Arc, Mutex, Weak},
};

/// Re-export capsule macro
#[cfg(feature = "capsule-macro")]
pub use rearch_capsule_macro::{capsule, factory};

mod side_effects;
pub use side_effects::*;

/// Performs a *consistent* read on all supplied capsules.
///
/// Consistency is important here: if you need the current data from a few different capsules,
/// *do not* read them individually, but rather group them together with one read!() call.
/// If you read capsules one at a time, there will be increased overhead in addition to possible
/// inconsistency (say if you read one capsule and then the container is updated right after).
///
/// Ideally, this would just be a function with variadic generics but Rust doesn't have them.
///
/// # Concurrency
/// Blocks when any of the requested capsules' data is not present in the container.
///
/// Internally, tries to read all supplied capsules with a read transaction first (cheap),
/// but if that fails (i.e., capsules' data not present in the container),
/// read!() spins up a write txn and initializes all needed capsules (which blocks).
/// TODO name this arc_read! and then add a read! attribute macro in other crate that returns refs
#[macro_export]
macro_rules! read {
    ($container:expr, $($C:ident),+) => {
        {
            paste::paste! {
                let container = &$container;
                #[allow(non_snake_case, unused_parens)]
                if let ($(Some([<i $C>])),+) =
                    container.with_read_txn(|txn| ($(txn.try_read::<$C>()),+)) {
                    ($([<i $C>]),+)
                } else {
                    container.with_write_txn(|txn| ($(txn.read_or_init::<$C>()),+))
                }
            }
        }
    };
}

/// Capsules are blueprints for creating some immutable data
/// and do not actually contain any data themselves.
/// See the README for more.
pub trait Capsule {
    /// The type of data associated with this capsule.
    // Associated type so that Capsule can only ever be implemented once for each concrete type
    type T: 'static + Send + Sync;

    /// Builds the capsule's immutable data using a given snapshot of the data flow graph.
    /// (The snapshot, a ContainerWriteTxn, is abstracted away for you.)
    ///
    /// ABSOLUTELY DO NOT TRIGGER ANY REBUILDS WITHIN THIS FUNCTION!
    /// Doing so will result in a deadlock.
    fn build(
        reader: &mut impl CapsuleReader<Self::T>,
        handle: &mut impl SideEffectHandle,
    ) -> Self::T;
}

pub trait CapsuleReader<T> {
    fn read<O: Capsule + 'static>(&mut self) -> Arc<O::T>;
    fn read_self(&self) -> Option<Arc<T>>;
}

pub trait SideEffectHandle {
    type Api: SideEffectHandleApi + 'static;

    // Registers the given side effect by initializing it on the first build and then returning
    // that same value on every subsequent build.
    // This allows you to store private mutable data that can be accessed on subsequent builds
    // in a deterministic way.
    fn register_side_effect<R: Send + Sync + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut Self::Api) -> R,
    ) -> Arc<R>;
}

pub trait SideEffectHandleApi {
    /// Provides a mechanism to trigger rebuilds.
    fn rebuilder<RebuildMutation: FnOnce()>(&self) -> impl Fn(RebuildMutation) + Send + Sync;
}

/// Containers store the current data within and the state of the data flow graph
/// created by capsules and their dependencies/dependents.
/// To read data from the container, it is suggested that you use the `read!()` macro.
/// See the README for more.
#[derive(Clone, Default)]
pub struct Container(Arc<ContainerStore>);
impl Container {
    /// Initializes a new `Container`.
    ///
    /// Containers contain no data when first created.
    /// Use `read!()` to populate and read some capsules!
    pub fn new() -> Self {
        Container(Arc::new(ContainerStore::default()))
    }

    /// Runs the supplied callback with a `ContainerReadTxn` that allows you to read
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read!()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    pub fn with_read_txn<R>(&self, to_run: impl FnOnce(&ContainerReadTxn) -> R) -> R {
        self.0.with_read_txn(to_run)
    }

    /// Runs the supplied callback with a `ContainerWriteTxn` that allows you to read and populate
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read!()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    ///
    /// This method blocks other writers (readers always have unrestricted access).
    ///
    /// ABSOLUTELY DO NOT trigger any capsule side effects (i.e., rebuilds) in the callback!
    /// This will result in a deadlock, and no future write transactions will be permitted.
    /// You can always trigger a rebuild in a new thread or after the ContainerWriteTxn drops.
    pub fn with_write_txn<R>(&self, to_run: impl FnOnce(&mut ContainerWriteTxn) -> R) -> R {
        let rebuilder = CapsuleRebuilder(Arc::downgrade(&self.0));
        self.0.with_write_txn(rebuilder, to_run)
    }
}

/// The internal backing store for a `Container`.
/// All capsule data is stored within `data`, and all data flow graph nodes are stored in `nodes`.
/// Keys for both are simply the `TypeId` of capsules, like `TypeId::of::<SomeCapsule>()`.
#[derive(Default)]
struct ContainerStore {
    data: concread::hashmap::HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    nodes: Mutex<std::collections::HashMap<TypeId, CapsuleManager>>,
}
impl ContainerStore {
    fn with_read_txn<R>(&self, to_run: impl FnOnce(&ContainerReadTxn) -> R) -> R {
        let txn = ContainerReadTxn {
            data: self.data.read(),
        };
        to_run(&txn)
    }

    fn with_write_txn<R>(
        &self,
        rebuilder: CapsuleRebuilder,
        to_run: impl FnOnce(&mut ContainerWriteTxn) -> R,
    ) -> R {
        let data = self.data.write();
        let nodes = &mut self.nodes.lock().expect("Mutex shouldn't fail to lock");
        let mut txn = ContainerWriteTxn {
            data,
            nodes,
            rebuilder,
        };

        let return_val = to_run(&mut txn);

        // We must commit the txn to avoid leaving the data and nodes in an inconsistent state
        txn.data.commit();

        return_val
    }
}

#[derive(Clone)]
struct CapsuleRebuilder(Weak<ContainerStore>);
impl CapsuleRebuilder {
    fn rebuild(&self, id: TypeId, mutation: impl FnOnce()) {
        if let Some(store) = self.0.upgrade() {
            store.with_write_txn(self.clone(), |txn| {
                // We have the txn now, so that means we also hold the data & nodes lock.
                // Thus, this is where we should run the supplied mutation.
                mutation();
                txn.rebuild_capsule_or_panic(id);
            });
        } else {
            // TODO log that C attempted to rebuild itself after container disposal
        }
    }
}

pub struct ContainerReadTxn<'a> {
    data: HashMapReadTxn<'a, TypeId, Arc<dyn Any + Send + Sync>>,
}
impl<'a> ContainerReadTxn<'a> {
    pub fn try_read<C: Capsule + 'static>(&self) -> Option<Arc<C::T>> {
        let capsule_type = TypeId::of::<C>();
        self.data.get(&capsule_type).map(|data| {
            data.clone()
                .downcast::<C::T>()
                .expect("Types should be properly enforced due to generics")
        })
    }
}

pub struct ContainerWriteTxn<'a> {
    data: HashMapWriteTxn<'a, TypeId, Arc<dyn Any + Send + Sync>>,
    nodes: &'a mut std::collections::HashMap<TypeId, CapsuleManager>,
    rebuilder: CapsuleRebuilder,
}
impl<'a> ContainerWriteTxn<'a> {
    pub fn try_read<C: Capsule + 'static>(&self) -> Option<Arc<C::T>> {
        let capsule_type = TypeId::of::<C>();
        self.data.get(&capsule_type).map(|data| {
            data.clone()
                .downcast::<C::T>()
                .expect("Types should be properly enforced due to generics")
        })
    }

    pub fn read_or_init<C: Capsule + 'static>(&mut self) -> Arc<C::T> {
        let capsule_type = TypeId::of::<C>();

        if !self.data.contains_key(&capsule_type) {
            self.build_capsule::<C>();
        }

        self.data
            .get(&capsule_type)
            .expect("Data should be present due to checking/building capsule above")
            .clone()
            .downcast::<C::T>()
            .expect("Types should be properly enforced due to generics")
    }
}

impl<'a> ContainerWriteTxn<'a> {
    /// Forcefully triggers a first build or rebuild for the supplied capsule
    fn build_capsule<C: Capsule + 'static>(&mut self) {
        let id = TypeId::of::<C>();

        // Ensure this capsule has a node for it in the graph
        self.nodes
            .entry(id)
            .or_insert_with(|| CapsuleManager::new::<C>(self.rebuilder.clone()));

        self.rebuild_capsule_or_panic(id);
    }

    /// Forcefully rebuild the capsule with the supplied id.
    /// Panics if the node with id is not in the graph, so this is only safe for rebuilds
    fn rebuild_capsule_or_panic(&mut self, id: TypeId) {
        self.build_single_node(&id);

        // Since we have already built the node above (since *it must be built in this method*),
        // we can skip it with skip(1) when we are handling the rest of the dependent subgraph
        let build_order = self.create_build_order(id).into_iter().skip(1);
        let build_order = self.garbage_collect_super_pure_nodes(build_order);
        build_order.iter().for_each(|id| self.build_single_node(id));
    }

    /// Gets the requested node or panics if it is not in the graph
    fn node_or_panic(&mut self, id: &TypeId) -> &mut CapsuleManager {
        self.nodes
            .get_mut(id)
            .expect("Requested node should be in the graph")
    }

    fn build_single_node(&mut self, id: &TypeId) {
        // Remove old dependency info since it will change on this build
        // We use std::mem::take below to prevent needing a clone on the existing dependencies
        let node = self.node_or_panic(id);
        let old_deps = std::mem::take(&mut node.dependencies);
        old_deps.iter().for_each(|dep| {
            self.node_or_panic(dep).dependents.remove(id);
        });

        // Trigger the build (which also populates its new dependencies in self)
        (self.node_or_panic(id).build)(self);
    }

    /// Function that creates this node's dependent subgraph build order (with self)
    fn create_build_order(&mut self, start: TypeId) -> Vec<TypeId> {
        let mut build_order = Vec::new();
        let mut stack = vec![start];
        let mut visited = HashSet::new();

        while let Some(id) = stack.pop() {
            visited.insert(id);
            build_order.push(id);

            self.node_or_panic(&id)
                .dependents
                .iter()
                .copied()
                .filter(|dep| !visited.contains(dep))
                .for_each(|dep| stack.push(dep));
        }

        build_order
    }

    /// Helper function that given a build_order, garbage collects all super pure nodes
    /// and returns the new build order without the (now garbage collected) super pure nodes
    fn garbage_collect_super_pure_nodes(
        &mut self,
        build_order: impl DoubleEndedIterator<Item = TypeId>,
    ) -> Vec<TypeId> {
        build_order
            .rev()
            .filter(|id| {
                let is_disposable = self.node_or_panic(id).is_disposable();
                if is_disposable {
                    let node = self.nodes.remove(id).expect("Node should be in graph");
                    node.dependencies.iter().for_each(|dep| {
                        self.node_or_panic(dep).dependents.remove(id);
                    });
                    self.data.remove(id);
                }
                !is_disposable
            })
            .rev()
            .collect::<Vec<_>>()
    }
}

// This struct is completely typeless in order to avoid *a lot* of dynamic dispatch
// that we used to have when dealing with the graph nodes.
// We avoid needing types by:
// - Storing our capsule's TypeId directly so that we can trigger rebuilds of our capsule
// - Storing a function pointer that performs the actual build
// Those are the only type-specific behaviors!
struct CapsuleManager {
    id: TypeId,
    dependencies: HashSet<TypeId>,
    dependents: HashSet<TypeId>,
    side_effect_data: Vec<Arc<dyn Any + Send + Sync>>,
    rebuilder: CapsuleRebuilder,
    build: fn(&mut ContainerWriteTxn),
}

impl CapsuleManager {
    fn new<C: Capsule + 'static>(rebuilder: CapsuleRebuilder) -> Self {
        CapsuleManager {
            id: TypeId::of::<C>(),
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            side_effect_data: Vec::new(),
            rebuilder,
            build: Self::build::<C>,
        }
    }

    fn build<C: Capsule + 'static>(txn: &mut ContainerWriteTxn) {
        let id = TypeId::of::<C>();
        // We need RefCell in order to safely do a double &mut borrow on txn
        let txn = &RefCell::new(txn);
        let new_data = C::build(
            &mut CapsuleReaderImpl::<C> {
                txn,
                ghost: PhantomData,
            },
            &mut CapsuleSideEffectHandleImpl {
                id: TypeId::of::<C>(),
                txn,
                index: 0,
            },
        );
        txn.borrow_mut().data.insert(id, Arc::new(new_data));
    }

    fn is_disposable(&self) -> bool {
        let is_super_pure = self.side_effect_data.is_empty();
        is_super_pure && self.dependents.is_empty()
    }
}

impl SideEffectHandleApi for CapsuleManager {
    fn rebuilder<RebuildMutation: FnOnce()>(&self) -> impl Fn(RebuildMutation) + Send + Sync {
        let id = self.id;
        let rebuilder = self.rebuilder.clone();
        move |mutation| rebuilder.rebuild(id, mutation)
    }
}

struct CapsuleReaderImpl<'txn_scope, 'txn_total, C: Capsule + 'static> {
    txn: &'txn_scope RefCell<&'txn_scope mut ContainerWriteTxn<'txn_total>>,
    ghost: PhantomData<C::T>,
}
impl<'txn_scope, 'txn_total, C: Capsule + 'static> CapsuleReader<C::T>
    for CapsuleReaderImpl<'txn_scope, 'txn_total, C>
{
    fn read<O: Capsule + 'static>(&mut self) -> Arc<O::T> {
        let (this, other) = (TypeId::of::<C>(), TypeId::of::<O>());
        if this == other {
            panic!(concat!(
                "A capsule tried depending upon itself (which isn't allowed)! ",
                "To read the current value of a capsule, instead use CapsuleReader::read_self()."
            ))
        }

        // Get the value (and make sure the other manager is initialized!)
        let mut txn = self.txn.borrow_mut();
        let data = txn.read_or_init::<O>();

        // Take care of some dependency housekeeping
        txn.node_or_panic(&other).dependents.insert(this);
        txn.node_or_panic(&this).dependencies.insert(other);

        data
    }

    fn read_self(&self) -> Option<Arc<C::T>> {
        self.txn.borrow().try_read::<C>()
    }
}

struct CapsuleSideEffectHandleImpl<'txn_scope, 'txn_total> {
    id: TypeId,
    txn: &'txn_scope RefCell<&'txn_scope mut ContainerWriteTxn<'txn_total>>,
    index: u16,
}
impl<'txn_scope, 'txn_total> SideEffectHandle
    for CapsuleSideEffectHandleImpl<'txn_scope, 'txn_total>
{
    type Api = CapsuleManager;
    fn register_side_effect<R: Send + Sync + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut Self::Api) -> R,
    ) -> Arc<R> {
        let mut txn = self.txn.borrow_mut();
        let manager = txn.node_or_panic(&self.id);
        if self.index as usize == manager.side_effect_data.len() {
            let data = side_effect(manager);
            manager.side_effect_data.push(Arc::new(data));
        };
        let data = manager.side_effect_data[self.index as usize].clone();
        self.index += 1;
        data.downcast::<R>().expect(concat!(
            "You cannot conditionally call side effects! ",
            "Always call your side effects unconditionally every build."
        ))
    }
}

#[cfg(test)]
mod tests {

    /// Check for Container: Send + Sync
    #[allow(unused)]
    mod container_thread_safe {
        use crate::*;
        struct SyncSendCheck<T: Send + Sync>(T);
        fn foo(bar: SyncSendCheck<Container>) {}
    }

    /// Check for some fundamental functionality with the classic count example
    mod basic_count {
        use crate::*;

        #[test]
        fn basic_count() {
            let container = Container::new();
            assert_eq!(
                1,
                *container.with_write_txn(|txn| txn.read_or_init::<CountPlusOneCapsule>())
            );
            assert_eq!(
                0,
                *container.with_read_txn(|txn| txn.try_read::<CountCapsule>().unwrap())
            )
        }

        #[test]
        fn read_macro() {
            let container = Container::new();
            let (count, count_plus_one) = read!(container, CountCapsule, CountPlusOneCapsule);
            assert_eq!(0, *count);
            assert_eq!(1, *count_plus_one);

            let count = read!(container, CountCapsule);
            assert_eq!(0, *count);

            let count_plus_one = read!(&Container::new(), CountPlusOneCapsule);
            assert_eq!(1, *count_plus_one);
        }

        struct CountCapsule;
        impl Capsule for CountCapsule {
            type T = u8;

            fn build(
                _: &mut impl CapsuleReader<Self::T>,
                _: &mut impl SideEffectHandle,
            ) -> Self::T {
                0
            }
        }
        struct CountPlusOneCapsule;
        impl Capsule for CountPlusOneCapsule {
            type T = u8;

            fn build(
                reader: &mut impl CapsuleReader<Self::T>,
                _: &mut impl SideEffectHandle,
            ) -> Self::T {
                reader.read::<CountCapsule>().as_ref() + 1
            }
        }
    }

    mod state_updates {
        use crate::*;

        #[test]
        fn state_gets_updates() {
            let container = Container::new();
            let (state, set_state) = &*read!(container, StateCapsule);
            assert_eq!(&0, state);
            set_state(1);
            let (state, set_state) = &*read!(container, StateCapsule);
            assert_eq!(&1, state);
            set_state(2);
            set_state(3);
            let (state, _) = &*read!(container, StateCapsule);
            assert_eq!(&3, state);
        }

        #[test]
        fn dependent_gets_updates() {
            let container = Container::new();

            let (state, plus_one) = read!(container, StateCapsule, DependentCapsule);
            let (state, set_state) = &*state;
            assert_eq!(&0, state);
            assert_eq!(1, *plus_one);
            set_state(1);

            let (state, plus_one) = read!(container, StateCapsule, DependentCapsule);
            let (state, _) = &*state;
            assert_eq!(&1, state);
            assert_eq!(2, *plus_one);
        }

        struct StateCapsule;
        impl Capsule for StateCapsule {
            type T = (u8, Box<dyn Fn(u8) + Send + Sync>);

            fn build(
                _: &mut impl CapsuleReader<Self::T>,
                handle: &mut impl SideEffectHandle,
            ) -> Self::T {
                let (state, set_state) = handle.state(0);
                (*state, Box::new(set_state))
            }
        }

        struct DependentCapsule;
        impl Capsule for DependentCapsule {
            type T = u8;

            fn build(
                reader: &mut impl CapsuleReader<Self::T>,
                _: &mut impl SideEffectHandle,
            ) -> Self::T {
                reader.read::<StateCapsule>().0 + 1
            }
        }
    }
}
