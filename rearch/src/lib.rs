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

/// Containers store the current data and state of the data flow graph created by capsules
/// and their dependencies/dependents.
/// To read data from the container, it is highly suggested that you use the `read!()` macro.
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

                // The node is guaranteed to be in the graph since this is a rebuild.
                // (To trigger a rebuild, a capsule must have used its side effect handle,
                // and using the side effect handle prevents the super pure gc.)
                txn.build_capsule_or_panic(id);
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
        let id = TypeId::of::<C>();
        self.data.get(&id).map(|data| {
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
        let id = TypeId::of::<C>();
        self.data.get(&id).map(|data| {
            data.clone()
                .downcast::<C::T>()
                .expect("Types should be properly enforced due to generics")
        })
    }

    pub fn read_or_init<C: Capsule + 'static>(&mut self) -> Arc<C::T> {
        let id = TypeId::of::<C>();
        if !self.data.contains_key(&id) {
            self.build_capsule::<C>();
        }
        self.try_read::<C>()
            .expect("Data should be present due to checking/building capsule above")
    }
}

impl<'a> ContainerWriteTxn<'a> {
    /// Triggers a first build or rebuild for the supplied capsule
    fn build_capsule<C: Capsule + 'static>(&mut self) {
        let id = TypeId::of::<C>();

        // Ensure this capsule has a node for it in the graph
        self.nodes
            .entry(id)
            .or_insert_with(|| CapsuleManager::new::<C>(self.rebuilder.clone()));

        self.build_capsule_or_panic(id);
    }

    /// Forcefully builds the capsule with the supplied id. Panics if node is not in the graph
    fn build_capsule_or_panic(&mut self, id: TypeId) {
        self.build_single_node(&id);

        // Since we have already built the node above (since *it must be built in this method*),
        // we can skip it with skip(1) when we are handling the rest of the dependent subgraph
        let build_order = self.create_build_order_stack(id).into_iter().rev().skip(1);
        let build_order = self.garbage_collect_diposable_nodes(build_order);
        build_order.iter().for_each(|id| self.build_single_node(id));
    }

    /// Gets the requested node or panics if it is not in the graph
    fn node_or_panic(&mut self, id: &TypeId) -> &mut CapsuleManager {
        self.nodes
            .get_mut(id)
            .expect("Requested node should be in the graph")
    }

    /// Builds only the requested node. Panics if node is not in the graph
    fn build_single_node(&mut self, id: &TypeId) {
        // Remove old dependency info since it may change on this build
        // We use std::mem::take below to prevent needing a clone on the existing dependencies
        let node = self.node_or_panic(id);
        let old_deps = std::mem::take(&mut node.dependencies);
        old_deps.iter().for_each(|dep| {
            self.node_or_panic(dep).dependents.remove(id);
        });

        // Trigger the build (which also populates its new dependencies in self)
        (self.node_or_panic(id).build)(self);
    }

    /// Creates the start node's dependent subgraph build order, including start, *as a stack*
    /// Thus, proper iteration order is done by popping off of the stack (in reverse order)!
    fn create_build_order_stack(&mut self, start: TypeId) -> Vec<TypeId> {
        // We need some more information alongside each node in order to do the topological sort
        // - False is for the first visit, which adds all deps to be visited and then self again
        // - True is for the second visit, which pushes node to the build order
        let mut to_visit_stack = vec![(false, start)];
        let mut visited = HashSet::new();
        let mut build_order_stack = Vec::new();

        while let Some((has_visited_before, node)) = to_visit_stack.pop() {
            if has_visited_before {
                // Already processed this node's dependents, so finally add to build order
                build_order_stack.push(node);
            } else if !visited.contains(&node) {
                // New node, so mark this node to be added later and process dependents
                visited.insert(node);
                to_visit_stack.push((true, node)); // mark node to be added to build order later
                self.node_or_panic(&node)
                    .dependents
                    .iter()
                    .copied()
                    .filter(|dep| !visited.contains(dep))
                    .for_each(|dep| to_visit_stack.push((false, dep)));
            }
        }

        build_order_stack
    }

    /// Helper function that given a build_order, garbage collects all super pure nodes
    /// that have no dependents (i.e., they are entirely disposable)
    /// and returns the new build order without the (now garbage collected) super pure nodes.
    /// While the build order specifies the order in which nodes must be built in to propagate
    /// updates, the reverse of the build order specifies the order in which we can trim down
    /// some fat through gc.
    fn garbage_collect_diposable_nodes(
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
            &mut CapsuleSideEffectHandleImpl { id, txn, index: 0 },
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
    ghost: PhantomData<C::T>, // phantom with C::T to prevent needing C to be Send + Sync
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

    #[cfg(feature = "capsule-macro")]
    mod complex_dependency_graph {
        use crate::{self as rearch, capsule, BuiltinSideEffects, Container, SideEffectHandle};

        // We use a more sophisticated graph here for a more thorough test of all functionality
        //
        // -> A -> B -> C -> D
        //      \      / \
        //  H -> E -> F -> G
        //
        // C, D, E, G, H are super pure. A, B, F are not.
        #[test]
        fn complex_dependency_graph() {
            let container = Container::new();
            let mut read_txn_counter = 0;

            container.with_read_txn(|txn| {
                read_txn_counter += 1;
                assert!(txn.try_read::<StatefulACapsule>().is_none());
                assert!(txn.try_read::<ACapsule>().is_none());
                assert!(txn.try_read::<BCapsule>().is_none());
                assert!(txn.try_read::<CCapsule>().is_none());
                assert!(txn.try_read::<DCapsule>().is_none());
                assert!(txn.try_read::<ECapsule>().is_none());
                assert!(txn.try_read::<FCapsule>().is_none());
                assert!(txn.try_read::<GCapsule>().is_none());
                assert!(txn.try_read::<HCapsule>().is_none());
            });

            rearch::read!(container, DCapsule, GCapsule);

            container.with_read_txn(|txn| {
                read_txn_counter += 1;
                assert!(txn.try_read::<StatefulACapsule>().is_some());
                assert_eq!(*txn.try_read::<ACapsule>().unwrap(), 0);
                assert_eq!(*txn.try_read::<BCapsule>().unwrap(), 1);
                assert_eq!(*txn.try_read::<CCapsule>().unwrap(), 2);
                assert_eq!(*txn.try_read::<DCapsule>().unwrap(), 2);
                assert_eq!(*txn.try_read::<ECapsule>().unwrap(), 1);
                assert_eq!(*txn.try_read::<FCapsule>().unwrap(), 1);
                assert_eq!(*txn.try_read::<GCapsule>().unwrap(), 3);
                assert_eq!(*txn.try_read::<HCapsule>().unwrap(), 1);
            });

            rearch::read!(container, StatefulACapsule).1(10);

            container.with_read_txn(|txn| {
                read_txn_counter += 1;
                assert!(txn.try_read::<StatefulACapsule>().is_some());
                assert_eq!(*txn.try_read::<ACapsule>().unwrap(), 10);
                assert_eq!(*txn.try_read::<BCapsule>().unwrap(), 11);
                assert_eq!(txn.try_read::<CCapsule>(), None);
                assert_eq!(txn.try_read::<DCapsule>(), None);
                assert_eq!(*txn.try_read::<ECapsule>().unwrap(), 11);
                assert_eq!(*txn.try_read::<FCapsule>().unwrap(), 11);
                assert_eq!(txn.try_read::<GCapsule>(), None);
                assert_eq!(*txn.try_read::<HCapsule>().unwrap(), 1);
            });

            assert_eq!(read_txn_counter, 3);
        }

        #[capsule]
        fn stateful_a(handle: &mut impl SideEffectHandle) -> (u8, Box<dyn Fn(u8) + Send + Sync>) {
            let (state, set_state) = handle.state(0);
            (*state, Box::new(set_state))
        }

        #[capsule]
        fn a(StatefulACapsule(a): StatefulACapsule) -> u8 {
            a.0
        }

        #[capsule]
        fn b(ACapsule(a): ACapsule, handle: &mut impl SideEffectHandle) -> u8 {
            handle.callonce(|| {});
            a + 1
        }

        #[capsule]
        fn c(BCapsule(b): BCapsule, FCapsule(f): FCapsule) -> u8 {
            b + f
        }

        #[capsule]
        fn d(CCapsule(c): CCapsule) -> u8 {
            *c
        }

        #[capsule]
        fn e(ACapsule(a): ACapsule, HCapsule(h): HCapsule) -> u8 {
            a + h
        }

        #[capsule]
        fn f(ECapsule(e): ECapsule, handle: &mut impl SideEffectHandle) -> u8 {
            handle.callonce(|| {});
            *e
        }

        #[capsule]
        fn g(CCapsule(c): CCapsule, FCapsule(f): FCapsule) -> u8 {
            c + f
        }

        #[capsule]
        fn h() -> u8 {
            1
        }
    }
}
