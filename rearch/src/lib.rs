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
pub use side_effects::BuiltinSideEffects;

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
/// TODO name this arc_read! and then add a read! attribute macro in other crate that returns refs?
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
    type T: 'static + Sync + Send;

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
    // Registers the given side effect by initializing it on the first build and then returning
    // that same value on every subsequent build.
    // This allows you to store private mutable data that can be accessed on subsequent builds
    // in a deterministic way.
    fn register_side_effect<R: Sync + Send + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut dyn SideEffectHandleApi) -> R,
    ) -> Arc<R>;
}

pub trait SideEffectHandleApi {
    /// Provides a mechanism to trigger rebuilds.
    fn rebuilder(&self) -> Box<dyn Fn() + Sync + Send>;
}

/// Containers store the current data within and the state of the data flow graph
/// created by capsules and their dependencies/dependents.
/// To read data from the container, it is suggested that you use the `read!()` macro.
/// See the README for more.
#[derive(Clone)]
pub struct Container(Arc<ContainerStore>);
impl Container {
    /// Initializes a new `Container`.
    ///
    /// Containers contain no data when first created.
    /// Use `read!()` to populate and read some capsules!
    pub fn new() -> Self {
        Container(Arc::new(ContainerStore::new()))
    }

    /// Runs the supplied callback with a `ContainerReadTxn` that allows you to read
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read!()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    pub fn with_read_txn<R>(&self, to_run: impl FnOnce(&ContainerReadTxn) -> R) -> R {
        let txn = ContainerReadTxn {
            data: self.0.data.read(),
        };
        to_run(&txn)
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
impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

/// The internal backing store for a `Container`.
/// All capsule data is stored within `data`, and all data flow graph nodes are stored in `nodes`.
/// Keys for both are simply the `TypeId` of capsules, like `TypeId::of::<SomeCapsule>()`.
struct ContainerStore {
    data: concread::hashmap::HashMap<TypeId, Arc<dyn Any + Sync + Send>>,
    nodes: Mutex<std::collections::HashMap<TypeId, Box<dyn DataFlowGraphNode + Sync + Send>>>,
}
impl ContainerStore {
    fn new() -> ContainerStore {
        ContainerStore {
            data: concread::hashmap::HashMap::new(),
            nodes: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn with_write_txn<R>(
        &self,
        rebuilder: CapsuleRebuilder,
        to_run: impl FnOnce(&mut ContainerWriteTxn) -> R,
    ) -> R {
        let data = self.data.write();
        let mut nodes = self.nodes.lock().expect("Mutex shouldn't fail to lock");
        let mut txn = ContainerWriteTxn {
            data,
            nodes: &mut nodes,
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
    fn rebuild<C: Capsule + 'static>(&self) {
        if let Some(store) = self.0.upgrade() {
            store.with_write_txn(self.clone(), |txn| txn.build_capsule::<C>());
        } else {
            // TODO log that C attempted to rebuild itself after container disposal
        }
    }
}

pub struct ContainerReadTxn<'a> {
    data: HashMapReadTxn<'a, TypeId, Arc<dyn Any + Sync + Send>>,
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
    data: HashMapWriteTxn<'a, TypeId, Arc<dyn Any + Sync + Send>>,
    nodes: &'a mut std::collections::HashMap<TypeId, Box<dyn DataFlowGraphNode + Sync + Send>>,
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

    /// Forcefully triggers a build/rebuild on the supplied capsule
    fn build_capsule<C: Capsule + 'static>(&mut self) {
        // This uses a little hack that works fairly well to maintain safety;
        // we remove node from graph and add it back in after to prevent a double &mut self borrow
        let capsule_type = TypeId::of::<C>();
        let mut node = self.nodes.remove(&capsule_type).unwrap_or_else(|| {
            Box::new(CapsuleManager::<C> {
                dependencies: HashSet::new(),
                dependents: HashSet::new(),
                capsule_rebuilder: self.rebuilder.clone(),
                side_effect_data: Vec::new(),
                ghost: PhantomData,
            })
        });
        node.build_dependent_subgraph(self);
        self.nodes.insert(capsule_type, node);
    }
}

struct CapsuleReaderImpl<'man, 'txn_scope, 'txn_total, C: Capsule + 'static> {
    manager: &'man RefCell<&'man mut CapsuleManager<C>>,
    txn: &'txn_scope mut ContainerWriteTxn<'txn_total>,
}
impl<'man, 'txn_scope, 'txn_total, C: Capsule + 'static> CapsuleReader<C::T>
    for CapsuleReaderImpl<'man, 'txn_scope, 'txn_total, C>
{
    fn read<O: Capsule + 'static>(&mut self) -> Arc<O::T> {
        let this_type = TypeId::of::<C>();
        let other_type = TypeId::of::<O>();

        if this_type == other_type {
            panic!(concat!(
                "A capsule tried depending upon itself (which isn't allowed)! ",
                "To read the current value of a capsule, instead use CapsuleReader::read_self()."
            ))
        }

        // Get the value (and make sure the other manager is initialized!)
        let data = self.txn.read_or_init::<O>();

        // Take care of some dependency housekeeping
        self.txn
            .nodes
            .get_mut(&other_type)
            .expect("Node should be initialized from read_or_init above")
            .add_dependent(this_type);
        self.manager.borrow_mut().dependencies.insert(other_type);

        data
    }

    fn read_self(&self) -> Option<Arc<C::T>> {
        self.txn.try_read::<C>()
    }
}

struct CapsuleSideEffectHandleImpl<'man, C: Capsule + 'static> {
    manager: &'man RefCell<&'man mut CapsuleManager<C>>,
    index: u16,
}
impl<'man, C: Capsule + 'static> SideEffectHandle for CapsuleSideEffectHandleImpl<'man, C> {
    fn register_side_effect<R: Sync + Send + 'static>(
        &mut self,
        side_effect: impl FnOnce(&mut dyn SideEffectHandleApi) -> R,
    ) -> Arc<R> {
        let mut manager = self.manager.borrow_mut();
        if self.index as usize == manager.side_effect_data.len() {
            let data = side_effect(*manager);
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

trait DataFlowGraphNode {
    fn build_self(&mut self, txn: &mut ContainerWriteTxn);
    fn build_dependent_subgraph(&mut self, txn: &mut ContainerWriteTxn);
    fn add_dependent(&mut self, dependent: TypeId);
    fn remove_dependent(&mut self, dependent: TypeId);
    fn dependents(&self) -> Vec<TypeId>;
    fn dependencies(&self) -> Vec<TypeId>;
    fn is_disposable(&self) -> bool;
}

struct CapsuleManager<C: Capsule + 'static> {
    dependencies: HashSet<TypeId>,
    dependents: HashSet<TypeId>,
    capsule_rebuilder: CapsuleRebuilder,
    side_effect_data: Vec<Arc<dyn Any + Sync + Send>>,
    // We use PhantomData with C::T to prevent needing the Capsule type itself to be Sync + Send
    // (T is already guaranteed to be Sync + Send so this just makes our lives easier)
    ghost: PhantomData<C::T>,
}

impl<C: Capsule + 'static> CapsuleManager<C> {
    /// Helper method that fetches the node with the specified id
    fn get_node<'a>(
        &'a mut self,
        txn: &'a mut ContainerWriteTxn,
        id: &TypeId,
    ) -> &'a mut dyn DataFlowGraphNode {
        if id == &TypeId::of::<C>() {
            self
        } else {
            txn.nodes
                .get_mut(id)
                .expect("All referenced nodes should be in the node graph")
                .as_mut()
        }
    }

    /// Helper function that creates this node's dependent subgraph build order (without self)
    fn create_build_order(&mut self, txn: &mut ContainerWriteTxn) -> Vec<TypeId> {
        let this_type = TypeId::of::<C>();
        let mut build_order = Vec::new();
        let mut stack = self.dependents();
        let mut visited = HashSet::new();
        visited.insert(this_type);

        while let Some(id) = stack.pop() {
            visited.insert(id);
            build_order.push(id);

            let node = self.get_node(txn, &id);
            let unvisited_dependents = node
                .dependents()
                .into_iter()
                .filter(|id| !visited.contains(id));
            stack.extend(unvisited_dependents);
        }

        build_order
    }

    /// Helper function that given a build_order, garbage collects all super pure nodes
    /// and returns the new build order without the (now garbage collected) super pure nodes
    fn garbage_collect_super_pure_nodes(
        &mut self,
        txn: &mut ContainerWriteTxn,
        build_order: Vec<TypeId>,
    ) -> Vec<TypeId> {
        build_order
            .into_iter()
            .rev()
            .filter(|id| {
                let node = self.get_node(txn, id);
                let is_disposable = node.is_disposable();

                if is_disposable {
                    node.dependencies()
                        .iter()
                        .for_each(|dep| self.get_node(txn, dep).remove_dependent(*id));
                    txn.nodes.remove(id);
                    txn.data.remove(id);
                }

                !is_disposable
            })
            .rev()
            .collect::<Vec<_>>()
    }
}

impl<C: Capsule + 'static> DataFlowGraphNode for CapsuleManager<C> {
    fn build_self(&mut self, txn: &mut ContainerWriteTxn) {
        let this_type = TypeId::of::<C>();

        // Clear up all dependency information from previous build
        // This will all be set by the CapsuleReader in this build
        self.dependencies().iter().for_each(|dep| {
            self.get_node(txn, dep).remove_dependent(this_type);
        });
        self.dependencies.clear();

        // Perform this build
        let self_ref = RefCell::new(self); // allows us to do a double &mut self borrow (safely)
        let new_data = C::build(
            &mut CapsuleReaderImpl {
                manager: &self_ref,
                txn,
            },
            &mut CapsuleSideEffectHandleImpl {
                manager: &self_ref,
                index: 0,
            },
        );

        // Update container with the result of this build
        txn.data.insert(this_type, Arc::new(new_data));
    }

    fn build_dependent_subgraph(&mut self, txn: &mut ContainerWriteTxn) {
        let build_order = self.create_build_order(txn);
        let build_order = self.garbage_collect_super_pure_nodes(txn, build_order);

        self.build_self(txn);
        build_order.iter().for_each(|id| {
            let mut node = txn
                .nodes
                .remove(id)
                .expect("Dependent nodes should be in the graph");
            node.build_self(txn);
            txn.nodes.insert(*id, node);
        });
    }

    fn add_dependent(&mut self, dependent: TypeId) {
        self.dependents.insert(dependent);
    }

    fn remove_dependent(&mut self, dependent: TypeId) {
        self.dependents.remove(&dependent);
    }

    fn is_disposable(&self) -> bool {
        let is_super_pure = self.side_effect_data.is_empty();
        is_super_pure && self.dependents.is_empty()
    }

    fn dependents(&self) -> Vec<TypeId> {
        self.dependents.iter().copied().collect()
    }

    fn dependencies(&self) -> Vec<TypeId> {
        self.dependencies.iter().copied().collect()
    }
}

impl<C: Capsule + 'static> SideEffectHandleApi for CapsuleManager<C> {
    fn rebuilder(&self) -> Box<dyn Fn() + Sync + Send> {
        let rebuilder = self.capsule_rebuilder.clone();
        Box::new(move || rebuilder.rebuild::<C>())
    }
}

#[cfg(test)]
mod tests {

    /// Check for Container: Sync + Send
    #[allow(unused)]
    mod container_thread_safe {
        use crate::*;
        struct SyncSendCheck<T: Sync + Send>(T);
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
            type T = (u8, Box<dyn Fn(u8) + Sync + Send>);

            fn build(
                _: &mut impl CapsuleReader<Self::T>,
                handle: &mut impl SideEffectHandle,
            ) -> Self::T {
                let (state, set_state) = handle.state(0);
                (*state, set_state)
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
