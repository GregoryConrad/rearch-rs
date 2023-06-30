#![warn(
    clippy::all,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::nursery,
    clippy::pedantic,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::clone_on_ref_ptr,
    clippy::unwrap_used
)]
#![feature(trait_upcasting)]
// TODO attempt to rewrite with paste::paste! to avoid needing this nightly feature
// (maybe just comment the nightly version out until this has stabilized)
#![feature(macro_metavar_expr)]
// TODO make these two opt-in via a temporary "better-api" feature that:
// - Requires nightly
// - Deprecates the (temporary) boring functions that are exposed for non-nightly use
#![feature(unboxed_closures)]
#![feature(fn_traits)]

use concread::hashmap::{HashMapReadTxn, HashMapWriteTxn};
use dyn_clone::DynClone;
use std::{
    any::{Any, TypeId},
    cell::OnceCell,
    collections::HashSet,
    sync::{Arc, Mutex, Weak},
};

#[cfg(feature = "macros")]
pub use rearch_macros::capsule;

pub mod side_effects;

/// Capsules are blueprints for creating some immutable data
/// and do not actually contain any data themselves.
/// See the README for more.
///
/// Note: *Do not manually implement this trait yourself!*
/// It is an internal implementation detail that may be changed or removed in the future.
// `Send` is required because `CapsuleManager` needs to store a copy of the capsule.
pub trait Capsule: Send + 'static {
    /// The type of data associated with this capsule.
    /// Capsule types must be `Clone + Send + Sync + 'static`.
    /// It is recommended to only put types with "cheap" clones in Capsules;
    /// think Copy types, small Vecs and other containers, basic data structures, and Arcs.
    /// If you are dealing with a bigger chunk of data, consider wrapping it in an Arc.
    /// Note: The `im` crate plays *very nicely* with rearch.
    // Associated type so that Capsule can only be implemented once for each concrete type
    type Data: CapsuleType;

    /// Builds the capsule's immutable data using a given snapshot of the data flow graph.
    /// (The snapshot, a `ContainerWriteTxn`, is abstracted away for you.)
    ///
    /// ABSOLUTELY DO NOT TRIGGER ANY REBUILDS WITHIN THIS FUNCTION!
    /// Doing so will result in a deadlock.
    fn build(&self, reader: CapsuleReader, effect: SideEffectRegistrar) -> Self::Data;
}

impl<T, F> Capsule for F
where
    T: CapsuleType,
    F: Fn(CapsuleReader, SideEffectRegistrar) -> T + Send + 'static,
{
    type Data = T;

    fn build(&self, reader: CapsuleReader, registrar: SideEffectRegistrar) -> Self::Data {
        self(reader, registrar)
    }
}

/// Represents a valid type of any capsule. Capsules must be `Clone + Send + Sync + 'static`.
pub trait CapsuleType: Any + DynClone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CapsuleType for T {}
dyn_clone::clone_trait_object!(CapsuleType);

/// Represents a side effect that can be utilized within the build method.
/// The key observation about side effects is that they form a tree, where each side effect:
/// - Has its own private state (including composing other side effects together)
/// - Presents some api to the build method, probably including a way to rebuild & update its state
// SideEffect needs a lifetime so that `Api` can contain a lifetime as well (if it needs to)
pub trait SideEffect: Send + 'static {
    /// The type exposed in the capsule build function when this side effect is registered;
    /// in other words, this is the api exposed by the side effect.
    ///
    /// Often, a side effect's api is a tuple, containing values like:
    /// - Data and/or state in this side effect
    /// - Function callbacks (perhaps to trigger a rebuild and/or update the side effect state)
    /// - Anything else imaginable!
    type Api<'a>
    where
        Self: 'a;

    // TODO inner type and function to make rebuild easier? Either that (which I prefer)
    // or we should provide a proc macro that generates a `impl SideEffect` from the following:
    //
    // #[side_effect(SyncPersistEffect<Read, Write, R, T>, (effect.data))]
    // fn sync_persist_effect_api<'a, Read, Write, R, T>(
    //     ((state, set_state), write): SyncPersistEffectInnerApi<'a>,
    // ) -> (&'a R, impl Fn(T) + Send + Sync + Clone + 'static)
    // where
    //     T: Send + 'static,
    //     R: Send + 'static,
    //     Read: FnOnce() -> R + Send + 'static,
    //     Write: Fn(T) -> R + Send + Sync + 'static,
    // {
    //     let write = write.clone();
    //     let persist = move |new_data| {
    //         let persist_result = write(new_data);
    //         set_state(persist_result);
    //     };
    //     (state, persist)
    // }

    /// Construct this side effect's build api, given:
    /// - A mutable reference to the current state of this side effect (&mut self)
    /// - A mechanism to trigger rebuilds that can also update the state of this side effect
    fn api(&mut self, rebuild: Box<dyn SideEffectRebuilder<Self>>) -> Self::Api<'_>;
}

// Using a trait object here to prevent a sea of complicated generics everywhere
// TODO maybe try making this static dispatch again? Box<dyn ...> is gross.
// (Keep inner as Box<dyn FnOnce(&mut S)>, but outer should be impl)
pub trait SideEffectRebuilder<S>:
    Fn(Box<dyn FnOnce(&mut S)>) + Send + Sync + DynClone + 'static
{
}
impl<S, F> SideEffectRebuilder<S> for F where
    F: Fn(Box<dyn FnOnce(&mut S)>) + Send + Sync + Clone + 'static
{
}
dyn_clone::clone_trait_object!(<S> SideEffectRebuilder<S>);

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
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(ContainerStore::default()))
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
    /// You can always trigger a rebuild in a new thread or after the `ContainerWriteTxn` drops.
    pub fn with_write_txn<R>(&self, to_run: impl FnOnce(&mut ContainerWriteTxn) -> R) -> R {
        let rebuilder = CapsuleRebuilder(Arc::downgrade(&self.0));
        self.0.with_write_txn(rebuilder, to_run)
    }

    /// Performs a *consistent* read on all supplied capsules.
    ///
    /// Consistency is important here: if you need the current data from a few different capsules,
    /// *do not* read them individually, but rather group them together with one `read()` call.
    /// If you read capsules one at a time, there will be increased overhead in addition to possible
    /// inconsistency (say if you read one capsule and then the container is updated right after).
    ///
    /// # Concurrency
    /// Blocks when any of the requested capsules' data is not present in the container.
    ///
    /// Internally, tries to read all supplied capsules with a read txn first (cheap),
    /// but if that fails (i.e., capsules' data not present in the container),
    /// spins up a write txn and initializes all needed capsules (which blocks).
    // TODO add our fun lil var args impl hack to Container to make it easier to read capsules
    pub fn read<CL: CapsuleList>(&self, capsules: CL) -> CL::Data {
        capsules.read(self)
    }
}

pub trait CapsuleList {
    type Data;
    fn read(self, container: &Container) -> Self::Data;
}

macro_rules! generate_capsule_list_impl {
    ($($C:ident),+) => {
        paste::paste! {
            #[allow(non_snake_case, unused_parens)]
            impl<$($C: Capsule),*> CapsuleList for ($($C),*) {
                type Data = ($($C::Data),*);
                fn read(self, container: &Container) -> Self::Data {
                    let ($([<i $C>]),*) = self;
                    if let ($(Some([<i $C>])),*) =
                        container.with_read_txn(|txn| ($(txn.try_read_raw::<$C>()),*)) {
                        ($([<i $C>]),*)
                    } else {
                        container.with_write_txn(|txn| ($(txn.read_or_init([<i $C>])),*))
                    }
                }
            }
        }
    };
}

generate_capsule_list_impl!(A);
generate_capsule_list_impl!(A, B);
generate_capsule_list_impl!(A, B, C);
generate_capsule_list_impl!(A, B, C, D);
generate_capsule_list_impl!(A, B, C, D, E);
generate_capsule_list_impl!(A, B, C, D, E, F);
generate_capsule_list_impl!(A, B, C, D, E, F, G);
generate_capsule_list_impl!(A, B, C, D, E, F, G, H);

/// The internal backing store for a `Container`.
/// All capsule data is stored within `data`, and all data flow graph nodes are stored in `nodes`.
/// Keys for both are simply the `TypeId` of capsules, like `TypeId::of::<SomeCapsule>()`.
#[derive(Default)]
struct ContainerStore {
    data: concread::hashmap::HashMap<TypeId, Box<dyn CapsuleType>>,
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
    fn rebuild(&self, id: TypeId, mutation: impl FnOnce(&mut CapsuleManager)) {
        #[allow(clippy::option_if_let_else)]
        if let Some(store) = self.0.upgrade() {
            #[cfg(feature = "logging")]
            log::debug!("Rebuilding Capsule ({:?})", id);

            // Note: The node is guaranteed to be in the graph here since this is a rebuild.
            // (And to trigger a rebuild, a capsule must have used its side effect handle,
            // and using the side effect handle prevents the super pure gc.)
            store.with_write_txn(self.clone(), |txn| {
                // We have the txn now, so that means we also hold the data & nodes lock.
                // Thus, this is where we should run the supplied mutation.
                mutation(txn.node_or_panic(id));
                txn.build_capsule_or_panic(id);
            });
        } else {
            #[cfg(feature = "logging")]
            log::warn!(
                "Rebuild triggered after Container disposal on Capsule ({:?})",
                id
            );
        }
    }
}

pub struct ContainerReadTxn<'a> {
    data: HashMapReadTxn<'a, TypeId, Box<dyn CapsuleType>>,
}
impl ContainerReadTxn<'_> {
    #[must_use]
    #[allow(unused_variables, clippy::needless_pass_by_value)]
    pub fn try_read<C: Capsule>(&self, capsule: C) -> Option<C::Data> {
        self.try_read_raw::<C>()
    }

    /// Tries a capsule read, but doesn't require an instance of the capsule itself
    fn try_read_raw<C: Capsule>(&self) -> Option<C::Data> {
        let id = TypeId::of::<C>();
        self.data.get(&id).map(|data| {
            let data: Box<dyn Any> = data.clone();
            *data
                .downcast::<C::Data>()
                .expect("Types should be properly enforced due to generics")
        })
    }
}

pub struct ContainerWriteTxn<'a> {
    data: HashMapWriteTxn<'a, TypeId, Box<dyn CapsuleType>>,
    nodes: &'a mut std::collections::HashMap<TypeId, CapsuleManager>,
    rebuilder: CapsuleRebuilder,
}
impl ContainerWriteTxn<'_> {
    #[must_use]
    #[allow(unused_variables, clippy::needless_pass_by_value)]
    pub fn try_read<C: Capsule>(&self, capsule: C) -> Option<C::Data> {
        self.try_read_raw::<C>()
    }

    pub fn read_or_init<C: Capsule>(&mut self, capsule: C) -> C::Data {
        let id = TypeId::of::<C>();
        if !self.data.contains_key(&id) {
            #[cfg(feature = "logging")]
            log::debug!("Initializing {} ({:?})", std::any::type_name::<C>(), id);

            self.build_capsule(capsule);
        }
        self.try_read_raw::<C>()
            .expect("Data should be present due to checking/building capsule above")
    }
}
impl ContainerWriteTxn<'_> {
    // TODO maybe we can expose a level-based Api to do this in just one method or two methods
    /*
    /// Attempts to garbage collect the given Capsule and its dependent subgraph, disposing
    /// the supplied Capsule and its dependent subgraph (and then returning `true`) only when
    /// the supplied Capsule and its dependent subgraph consist only of super pure capsules.
    // TODO what about when node isnt in container? probs should return custom enum
    pub fn try_garbage_collect_super_pure<C: Capsule>(&mut self) -> bool {
        let id = TypeId::of::<C>();
        let build_order = self.create_build_order_stack(id);

        let is_all_super_pure = build_order
            .iter()
            .all(|id| self.node_or_panic(*id).is_super_pure());

        if is_all_super_pure {
            for id in build_order {
                self.dispose_single_node(id);
            }
        }

        is_all_super_pure
    }
    */

    /*
    /// Attempts to garbage collect the given Capsule and its dependent subgraph, disposing
    /// the supplied Capsule and its dependent subgraph (and then returning `true`) only when:
    /// - The dependent subgraph consists only of super pure capsules, or
    /// - `dispose_impure_dependents` is set to true
    ///
    /// If you are not expecting the supplied Capsule to have dependents,
    /// _set `dispose_impure_dependents` to false_, as setting it to true is *highly* unsafe.
    /// In addition, in this case, it is also recommended to `assert!` the return value of this
    /// function is true to ensure you didn't accidentally create other Capsule(s) which depend
    /// on the supplied Capsule.
    ///
    /// # Safety
    /// This is inherently unsafe because it violates the contract that capsules which
    /// are not super pure will not be disposed, at least prior to their Container's disposal.
    /// While invoking this method will never result in undefined behavior,
    /// it can *easily* result in logic bugs, thus the unsafe marking.
    /// This method is only exposed for the *very* few and specific use cases in which there
    /// is a need to deeply integrate with rearch in order to prevent leaks,
    /// such as when developing a UI framework and you need to listen to capsule updates.
    // TODO consider splitting this into different methods, _single, _sp_deps, _ip_deps
    pub unsafe fn force_garbage_collect<C: Capsule>(
        dispose_impure_dependents: bool,
    ) -> bool {
        // handles these cases:
        // - super pure, with impure dependents
        // - impure, no dependents
        // - impure, with super pure dependents
        // - impure, with impure dependents
        todo!()
    }
    */
}
impl ContainerWriteTxn<'_> {
    /// Tries a capsule read, but doesn't require an instance of the capsule itself
    fn try_read_raw<C: Capsule>(&self) -> Option<C::Data> {
        let id = TypeId::of::<C>();
        self.data.get(&id).map(|data| {
            let data: Box<dyn Any> = data.clone();
            *data
                .downcast::<C::Data>()
                .expect("Types should be properly enforced due to generics")
        })
    }

    /// Triggers a first build or rebuild for the supplied capsule
    fn build_capsule<C: Capsule>(&mut self, capsule: C) {
        let id = TypeId::of::<C>();

        // Ensure this capsule has a node for it in the graph
        if let std::collections::hash_map::Entry::Vacant(e) = self.nodes.entry(id) {
            let rebuilder = self.rebuilder.clone();
            let manager = CapsuleManager::new(capsule, rebuilder);
            e.insert(manager);
        }

        self.build_capsule_or_panic(id);
    }

    /// Forcefully builds the capsule with the supplied id. Panics if node is not in the graph
    fn build_capsule_or_panic(&mut self, id: TypeId) {
        self.build_single_node(id);

        // Since we have already built the node above (since *it must be built in this method*),
        // we can skip it with skip(1) when we are handling the rest of the dependent subgraph
        let build_order = {
            let build_order = self.create_build_order_stack(id).into_iter().rev().skip(1);
            self.garbage_collect_diposable_nodes(build_order)
        };
        for id in build_order {
            self.build_single_node(id);
        }
    }

    /// Gets the requested node or panics if it is not in the graph
    fn node_or_panic(&mut self, id: TypeId) -> &mut CapsuleManager {
        self.nodes
            .get_mut(&id)
            .expect("Requested node should be in the graph")
    }

    /// Builds only the requested node. Panics if the node is not in the graph
    fn build_single_node(&mut self, id: TypeId) {
        // Remove old dependency info since it may change on this build
        // We use std::mem::take below to prevent needing a clone on the existing dependencies
        let node = self.node_or_panic(id);
        let old_deps = std::mem::take(&mut node.dependencies);
        for dep in old_deps {
            self.node_or_panic(dep).dependents.remove(&id);
        }

        // Trigger the build (which also populates its new dependencies in self)
        (self.node_or_panic(id).build)(self);
    }

    /// Forcefully disposes only the requested node, cleaning up the dependency graph as needed.
    /// Panics if the node is not in the graph.
    fn dispose_single_node(&mut self, id: TypeId) {
        self.data.remove(&id);
        self.nodes
            .remove(&id)
            .expect("Node should be in graph")
            .dependencies
            .iter()
            .for_each(|dep| {
                self.node_or_panic(*dep).dependents.remove(&id);
            });
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
                self.node_or_panic(node)
                    .dependents
                    .iter()
                    .copied()
                    .filter(|dep| !visited.contains(dep))
                    .for_each(|dep| to_visit_stack.push((false, dep)));
            }
        }

        build_order_stack
    }

    /// Helper function that given a `build_order`, garbage collects all super pure nodes
    /// that have no dependents (i.e., they are entirely disposable)
    /// and returns the new build order without the (now garbage collected) super pure nodes.
    /// While the build order specifies the order in which nodes must be built in to propagate
    /// updates, the reverse of the build order specifies the order in which we can trim down
    /// some fat through gc.
    fn garbage_collect_diposable_nodes(
        &mut self,
        build_order: impl DoubleEndedIterator<Item = TypeId>,
    ) -> impl DoubleEndedIterator<Item = TypeId> {
        let mut non_disposable = Vec::new();

        build_order.rev().for_each(|id| {
            let is_disposable = self.node_or_panic(id).is_disposable();
            if is_disposable {
                self.dispose_single_node(id);
            } else {
                non_disposable.push(id);
            }
        });

        non_disposable.into_iter().rev()
    }
}

// This struct is completely typeless in order to avoid *a lot* of dynamic dispatch
// that we used to have when dealing with the graph nodes.
// We avoid needing types by storing a fn pointer of a function that performs the actual build.
// A capsule's build is a capsule's only type-specific behavior!
// Note: we use Option over a few fields below to enforce a safer memory model of ownership
// (ownership of some of the CapsuleManager's fields must be taken during builds).
const EX_OWNER_MSG: &str =
    "Attempted to use a CapsuleManager field when someone else already had ownership";
struct CapsuleManager {
    capsule: Option<Box<dyn Any + Send>>,
    dependencies: HashSet<TypeId>,
    dependents: HashSet<TypeId>,
    side_effect: Option<OnceCell<Box<dyn Any + Send>>>,
    rebuilder: CapsuleRebuilder,
    build: fn(&mut ContainerWriteTxn),
}
impl CapsuleManager {
    fn new<C: Capsule>(capsule: C, rebuilder: CapsuleRebuilder) -> Self {
        Self {
            capsule: Some(Box::new(capsule)),
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            side_effect: Some(OnceCell::new()),
            rebuilder,
            build: Self::build::<C>,
        }
    }

    fn build<C: Capsule>(txn: &mut ContainerWriteTxn) {
        let id = TypeId::of::<C>();

        #[cfg(feature = "logging")]
        log::trace!("Building {} ({:?})", std::any::type_name::<C>(), id);

        let manager = txn.node_or_panic(id);
        let capsule = std::mem::take(&mut manager.capsule).expect(EX_OWNER_MSG);
        let mut side_effect = std::mem::take(&mut manager.side_effect).expect(EX_OWNER_MSG);
        let rebuilder = {
            let rebuilder = manager.rebuilder.clone();
            Box::new(move |mutation: Box<dyn FnOnce(&mut Box<_>)>| {
                rebuilder.rebuild(id, |manager| {
                    let effect = manager.side_effect.as_mut().expect(EX_OWNER_MSG);
                    let effect = effect.get_mut().expect(concat!(
                        "The side effect must've been previously initialized ",
                        "in order to use the rebuilder"
                    ));
                    mutation(effect);
                });
            })
        };

        let new_data = C::build(
            capsule
                .downcast_ref::<C>()
                .expect("Types should be properly enforced due to generics"),
            CapsuleReader { id, txn },
            SideEffectRegistrar::new(&mut side_effect, rebuilder),
        );

        let manager = txn.node_or_panic(id);
        manager.capsule = Some(capsule);
        manager.side_effect = Some(side_effect);

        txn.data.insert(id, Box::new(new_data));
    }

    fn is_super_pure(&self) -> bool {
        self.side_effect
            .as_ref()
            .expect(EX_OWNER_MSG)
            .get()
            .is_none()
    }

    fn is_disposable(&self) -> bool {
        self.is_super_pure() && self.dependents.is_empty()
    }
}

/// Allows you to read the current data of capsules based on the given state of the container.
pub struct CapsuleReader<'scope, 'total> {
    id: TypeId,
    txn: &'scope mut ContainerWriteTxn<'total>,
    // TODO mock utility, like MockCapsuleReaderBuilder::new().set(capsule, value).set(...).build()
    // #[cfg(feature = "capsule-reader-mock")]
    // mock: Option<CapsuleMocks>,
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

impl CapsuleReader<'_, '_> {
    /// Reads the current data of the supplied capsule, initializing it if needed.
    /// Internally forms a dependency graph amongst capsules, so feel free to conditionally invoke
    /// this function in case you only conditionally need a capsule's value.
    ///
    /// # Panics
    /// Panics when a capsule attempts to read itself in its first build.
    pub fn read<C: Capsule>(&mut self, capsule: C) -> C::Data {
        let (this, other) = (self.id, TypeId::of::<C>());
        if this == other {
            return self.txn.try_read(capsule).unwrap_or_else(|| {
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

/// Registers the given side effect and returns its build api.
/// You can only call register once on purpose (it consumes self);
/// to register multiple side effects, simply pass them in together!
/// If you have a super pure capsule that you wish to make not super pure,
/// simply call `register()` with no arguments.
pub struct SideEffectRegistrar<'a> {
    side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
    rebuild: Box<dyn SideEffectRebuilder<Box<dyn Any + Send>>>,
}
impl<'a> SideEffectRegistrar<'a> {
    /// Creates a new `SideEffectRegistrar`.
    ///
    /// This is public only to enable easier mocking in your code;
    /// do not use this method in a non-test context.
    pub fn new(
        side_effect: &'a mut OnceCell<Box<dyn Any + Send>>,
        rebuild: Box<dyn SideEffectRebuilder<Box<dyn Any + Send>>>,
    ) -> Self {
        Self {
            side_effect,
            rebuild,
        }
    }
}

const EFFECT_FAILED_CAST_MSG: &str =
    "The SideEffect registered with SideEffectRegistrar cannot be changed!";

// Empty register() for the no-op side effect
impl FnOnce<()> for SideEffectRegistrar<'_> {
    type Output = ();
    extern "rust-call" fn call_once(self, _: ()) -> Self::Output {
        // Initialize with the no-op side effect
        self.side_effect.get_or_init(|| Box::new(()));

        // Ensure side effect wasn't changed
        assert!(
            self.side_effect
                .get_mut()
                .expect("Side effect should've been initialized above")
                .is::<()>(),
            "You cannot change the side effect(s) passed to register()!"
        );
    }
}

macro_rules! generate_side_effect_registrar_fn_impl {
    ($($types:ident),+) => {
        #[allow(unused_parens, non_snake_case)]
        impl<'a, $($types: SideEffect),*> FnOnce<($($types,)*)> for SideEffectRegistrar<'a> {
            type Output = ($($types::Api<'a>),*);

            extern "rust-call" fn call_once(self, args: ($($types,)*)) -> Self::Output {
                let ($($types,)*) = args;
                self.side_effect.get_or_init(|| Box::new(($($types),*)));
                let effect = self
                    .side_effect
                    .get_mut()
                    .expect("Side effect should've been initialized above")
                    .downcast_mut::<($($types),*)>()
                    .expect(EFFECT_FAILED_CAST_MSG);

                effect.api(Box::new(move |mutation| {
                    (self.rebuild)(Box::new(|effect| {
                        let effect = effect
                            .downcast_mut::<($($types),*)>()
                            .expect(EFFECT_FAILED_CAST_MSG);
                        mutation(effect);
                    }));
                }))
            }
        }
    }
}
generate_side_effect_registrar_fn_impl!(A);
generate_side_effect_registrar_fn_impl!(A, B);
generate_side_effect_registrar_fn_impl!(A, B, C);
generate_side_effect_registrar_fn_impl!(A, B, C, D);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G);
generate_side_effect_registrar_fn_impl!(A, B, C, D, E, F, G, H);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::missing_const_for_fn)]
mod tests {

    /// Check for Container: Send + Sync
    #[allow(unused)]
    mod container_thread_safe {
        use crate::*;
        struct SyncSendCheck<T: Send + Sync>(T);
        const fn foo(bar: &SyncSendCheck<Container>) {}
    }

    /// Check for some fundamental functionality with the classic count example
    #[test]
    fn basic_count() {
        use crate::*;

        fn count(_: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            0
        }
        fn count_plus_one(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(count) + 1
        }

        let container = Container::new();
        assert_eq!(
            (None, None),
            container.with_read_txn(|txn| (txn.try_read(count), txn.try_read(count_plus_one)))
        );
        assert_eq!(
            1,
            container.with_write_txn(|txn| txn.read_or_init(count_plus_one))
        );
        assert_eq!(
            0,
            container.with_read_txn(|txn| txn.try_read(count).unwrap())
        );

        let container = Container::new();
        assert_eq!((0, 1), container.read((count, count_plus_one)));
    }

    mod state_updates {
        use crate::*;

        #[test]
        fn state_gets_updates() {
            let container = Container::new();

            let (state, set_state) = container.read(stateful);
            assert_eq!(state, 0);

            set_state(1);
            let (state, set_state) = container.read(stateful);
            assert_eq!(state, 1);

            set_state(2);
            set_state(3);
            let (state, _) = container.read(stateful);
            assert_eq!(state, 3);
        }

        #[test]
        fn dependent_gets_updates() {
            let container = Container::new();

            let ((state, set_state), plus_one) = container.read((stateful, dependent));
            assert_eq!(0, state);
            assert_eq!(1, plus_one);
            set_state(1);

            let ((state, _), plus_one) = container.read((stateful, dependent));
            assert_eq!(1, state);
            assert_eq!(2, plus_one);
        }

        fn stateful(
            _: CapsuleReader,
            register: SideEffectRegistrar,
        ) -> (u8, std::sync::Arc<dyn Fn(u8) + Send + Sync>) {
            let (state, set_state) = register(side_effects::StateEffect::new(0));
            (*state, set_state)
        }

        fn dependent(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(stateful).0 + 1
        }
    }

    // We use a more sophisticated graph here for a more thorough test of all functionality
    //
    // -> A -> B -> C -> D
    //      \      / \
    //  H -> E -> F -> G
    //
    // C, D, E, G, H are super pure. A, B, F are not.
    #[test]
    fn complex_dependency_graph() {
        use crate::{side_effects, CapsuleReader, Container, SideEffectRegistrar};

        fn stateful_a(
            _: CapsuleReader,
            register: SideEffectRegistrar,
        ) -> (u8, std::sync::Arc<dyn Fn(u8) + Send + Sync>) {
            let (state, set_state) = register(side_effects::StateEffect::new(0));
            (*state, set_state)
        }

        fn a(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(stateful_a).0
        }

        fn b(mut get: CapsuleReader, register: SideEffectRegistrar) -> u8 {
            register(());
            get(a) + 1
        }

        fn c(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(b) + get(f)
        }

        fn d(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(c)
        }

        fn e(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(a) + get(h)
        }

        fn f(mut get: CapsuleReader, register: SideEffectRegistrar) -> u8 {
            register(());
            get(e)
        }

        fn g(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(c) + get(f)
        }

        fn h(_: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            1
        }

        let container = Container::new();
        let mut read_txn_counter = 0;

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(stateful_a).is_none());
            assert_eq!(txn.try_read(a), None);
            assert_eq!(txn.try_read(b), None);
            assert_eq!(txn.try_read(c), None);
            assert_eq!(txn.try_read(d), None);
            assert_eq!(txn.try_read(e), None);
            assert_eq!(txn.try_read(f), None);
            assert_eq!(txn.try_read(g), None);
            assert_eq!(txn.try_read(h), None);
        });

        container.read((d, g));

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(stateful_a).is_some());
            assert_eq!(txn.try_read(a).unwrap(), 0);
            assert_eq!(txn.try_read(b).unwrap(), 1);
            assert_eq!(txn.try_read(c).unwrap(), 2);
            assert_eq!(txn.try_read(d).unwrap(), 2);
            assert_eq!(txn.try_read(e).unwrap(), 1);
            assert_eq!(txn.try_read(f).unwrap(), 1);
            assert_eq!(txn.try_read(g).unwrap(), 3);
            assert_eq!(txn.try_read(h).unwrap(), 1);
        });

        container.read(stateful_a).1(10);

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(stateful_a).is_some());
            assert_eq!(txn.try_read(a).unwrap(), 10);
            assert_eq!(txn.try_read(b).unwrap(), 11);
            assert_eq!(txn.try_read(c), None);
            assert_eq!(txn.try_read(d), None);
            assert_eq!(txn.try_read(e).unwrap(), 11);
            assert_eq!(txn.try_read(f).unwrap(), 11);
            assert_eq!(txn.try_read(g), None);
            assert_eq!(txn.try_read(h).unwrap(), 1);
        });

        assert_eq!(read_txn_counter, 3);
    }
}
