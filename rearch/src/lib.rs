#![cfg_attr(feature = "experimental-api", feature(unboxed_closures, fn_traits))]

use parking_lot::ReentrantMutex;
use std::{
    any::Any,
    cell::{OnceCell, RefCell},
    collections::HashSet,
    ops::Deref,
    sync::{Arc, Mutex, Weak},
};

mod capsule_key;
pub use capsule_key::CapsuleKey;
pub(crate) use capsule_key::{CapsuleId, CreateCapsuleId};

mod capsule_reader;
pub use capsule_reader::{CapsuleReader, MockCapsuleReaderBuilder};

mod side_effect_registrar;
pub use side_effect_registrar::SideEffectRegistrar;

mod txn;
#[cfg(feature = "experimental-txn")]
pub use txn::{ContainerReadTxn, ContainerWriteTxn};
#[cfg(not(feature = "experimental-txn"))]
use txn::{ContainerReadTxn, ContainerWriteTxn};

/// Capsules are blueprints for creating some immutable data
/// and do not actually contain any data themselves.
/// See the documentation for more.
// - `Send` is required because `CapsuleManager` needs to store a copy of the capsule
// - `'static` is required to store a copy of the capsule, and for `TypeId::of()`
pub trait Capsule: Send + 'static {
    /// The type of data associated with this capsule, which must be `Send + Sync + 'static`.
    ///
    /// [`Capsule::Data`] that implements `Clone` will also unlock a few convenience methods.
    ///
    /// Note: when your types do implement `Clone`, it is suggested to be a "cheap" Clone.
    /// `Arc`s, small collections/data structures, and the `im` crate are great for this.
    type Data: Send + Sync + 'static;

    /// Builds the capsule's immutable data using a given snapshot of the data flow graph.
    /// (The snapshot, a `ContainerWriteTxn`, is abstracted away for you via [`CapsuleHandle`].)
    ///
    /// # Concurrency
    /// ABSOLUTELY DO NOT TRIGGER ANY REBUILDS WITHIN THIS FUNCTION!
    /// Doing so may result in a deadlock or a panic.
    fn build(&self, handle: CapsuleHandle) -> Self::Data;

    /// Returns whether or not a capsule's old data and new data are equivalent
    /// (and thus whether or not we can skip rebuilding dependents as an optimization).
    fn eq(old: &Self::Data, new: &Self::Data) -> bool;

    /// Returns the key to use for this capsule.
    /// Most capsules should use the default implementation,
    /// which is for static capsules.
    /// If you specifically need dynamic capsules,
    /// such as for an incremental computation focused application,
    /// you will need to implement this function.
    /// See [`CapsuleKey`] for more.
    fn key(&self) -> CapsuleKey {
        CapsuleKey::default()
    }
}
impl<T, F> Capsule for F
where
    T: Send + Sync + 'static,
    F: Fn(CapsuleHandle) -> T + Send + 'static,
{
    type Data = T;

    fn build(&self, handle: CapsuleHandle) -> Self::Data {
        self(handle)
    }

    // Unfortunately, negative trait impls don't exist yet.
    // If they did, this would have a separate impl for T: Eq.
    fn eq(_old: &Self::Data, _new: &Self::Data) -> bool {
        false
    }
}

/// Shorthand for `Clone + Send + Sync + 'static`,
/// which makes returning `impl Trait` far easier from capsules,
/// where `Trait` is often an `Fn` from side effects.
pub trait CData: Clone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CData for T {}

/// The handle given to [`Capsule`]s in order to [`Capsule::build`] their [`Capsule::Data`].
/// See [`CapsuleReader`] and [`SideEffectRegistrar`] for more.
pub struct CapsuleHandle<'txn_scope, 'txn_total, 'build> {
    pub get: CapsuleReader<'txn_scope, 'txn_total>,
    pub register: SideEffectRegistrar<'build>,
}

/// Represents a side effect that can be utilized within the build function.
///
/// The key observation about side effects is that they form a tree, where each side effect:
/// - Has its own private state (including composing other side effects together)
/// - Presents some api to the build method, probably including a way to rebuild & update its state
pub trait SideEffect {
    /// The type exposed in the capsule build function when this side effect is registered;
    /// in other words, this is the api exposed by the side effect.
    ///
    /// Often, a side effect's api is a tuple, containing values like:
    /// - Data and/or state in this side effect
    /// - Function callbacks (perhaps to trigger a rebuild and/or update the side effect state)
    /// - Anything else imaginable!
    type Api<'registrar>;

    /// Construct this side effect's `Api` via the given [`SideEffectRegistrar`].
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_>;
}
impl<T, F: FnOnce(SideEffectRegistrar) -> T> SideEffect for F {
    type Api<'registrar> = T;
    fn build(self, registrar: SideEffectRegistrar) -> Self::Api<'_> {
        self(registrar)
    }
}
const EFFECT_FAILED_CAST_MSG: &str =
    "You cannot change the side effect(s) passed to SideEffectRegistrar::register()!";
// These should be declarative macros, but they unfortunately would require macro_metavar_expr
rearch_macros::generate_tuple_side_effect_impl!(); // () is the no-op side effect
rearch_macros::generate_tuple_side_effect_impl!(A B);
rearch_macros::generate_tuple_side_effect_impl!(A B C);
rearch_macros::generate_tuple_side_effect_impl!(A B C D);
rearch_macros::generate_tuple_side_effect_impl!(A B C D E);
rearch_macros::generate_tuple_side_effect_impl!(A B C D E F);
rearch_macros::generate_tuple_side_effect_impl!(A B C D E F G);
rearch_macros::generate_tuple_side_effect_impl!(A B C D E F G H);

/// Containers store the current data and state of the data flow graph created by capsules
/// and their dependencies/dependents.
/// See the README for more.
#[derive(Clone, Default)]
pub struct Container(Arc<ContainerStore>);
impl Container {
    /// Initializes a new `Container`.
    ///
    /// Containers contain no data when first created.
    /// Use `read()` to populate and read some capsules!
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Performs a *consistent* read on all supplied capsules that have cloneable data.
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
    pub fn read<Capsules: CapsulesWithCloneRead>(&self, capsules: Capsules) -> Capsules::Data {
        capsules.read(self)
    }

    /// Provides a mechanism to *temporarily* listen to changes in some capsule(s).
    /// The provided listener is called once at the time of the listener's registration,
    /// and then once again everytime a dependency changes.
    ///
    /// Returns a `ListenerHandle`, which doesn't do anything other than implement Drop,
    /// and its Drop implementation will remove `listener` from the Container.
    ///
    /// Thus, if you want the handle to live for as long as the Container itself,
    /// it is instead recommended to create a non-idempotent capsule
    /// (use the `effects::as_listener()` side effect)
    /// that acts as your listener. When you normally would call `Container::listen()`,
    /// instead call `container.read(my_non_idempotent_listener)` to initialize it.
    ///
    /// # Panics
    /// Panics if you attempt to register the same listener twice,
    /// before the first `ListenerHandle` is dropped.
    #[must_use]
    pub fn listen<Effect, EffectFactory, Listener>(
        &self,
        effect_factory: EffectFactory,
        listener: Listener,
    ) -> ListenerHandle
    where
        Effect: SideEffect,
        EffectFactory: 'static + Send + Fn() -> Effect,
        Listener: Fn(CapsuleReader, <Effect as SideEffect>::Api<'_>) + Send + 'static,
    {
        // We make a temporary non-idempotent capsule for the listener so that
        // it doesn't get disposed by the idempotent gc
        let tmp_capsule = move |CapsuleHandle { get, register }: CapsuleHandle| {
            let effect = effect_factory();
            let effect_api = register.register(effect);
            listener(get, effect_api);
        };
        let id = tmp_capsule.id();

        // Put the temporary capsule into the container to listen to updates
        self.with_write_txn(move |txn| {
            assert_eq!(
                txn.try_read(&tmp_capsule),
                None,
                "You cannot pass the same listener into Container::listen() {}",
                "until the original returned ListenerHandle is dropped!"
            );
            txn.read_or_init(tmp_capsule);
        });

        ListenerHandle {
            id,
            store: Arc::downgrade(&self.0),
        }
    }
}
impl Container {
    /// Runs the supplied callback with a `ContainerReadTxn` that allows you to read
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    #[cfg(not(feature = "experimental-txn"))]
    fn with_read_txn<'f, R>(&self, to_run: impl 'f + FnOnce(&ContainerReadTxn) -> R) -> R {
        self.0.with_read_txn(to_run)
    }
    #[cfg(feature = "experimental-txn")]
    pub fn with_read_txn<'f, R>(&self, to_run: impl 'f + FnOnce(&ContainerReadTxn) -> R) -> R {
        self.0.with_read_txn(to_run)
    }

    /// Runs the supplied callback with a `ContainerWriteTxn` that allows you to read and populate
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    ///
    /// This method blocks other writers (readers always have unrestricted access).
    ///
    /// ABSOLUTELY DO NOT trigger any capsule side effects (i.e., rebuilds) in the callback!
    /// This will result in a deadlock, and no future write transactions will be permitted.
    /// You can always trigger a rebuild in a new thread or after the `ContainerWriteTxn` drops.
    #[cfg(not(feature = "experimental-txn"))]
    fn with_write_txn<'f, R>(&self, to_run: impl 'f + FnOnce(&mut ContainerWriteTxn) -> R) -> R {
        let orchestrator = SideEffectTxnOrchestrator(Arc::downgrade(&self.0));
        self.0.with_write_txn(orchestrator, to_run)
    }
    #[cfg(feature = "experimental-txn")]
    pub fn with_write_txn<'f, R>(
        &self,
        to_run: impl 'f + FnOnce(&mut ContainerWriteTxn) -> R,
    ) -> R {
        let orchestrator = SideEffectTxnOrchestrator(Arc::downgrade(&self.0));
        self.0.with_write_txn(orchestrator, to_run)
    }
}

/// Represents a handle onto a particular listener, as created with [`Container::listen`].
///
/// This struct doesn't do anything other than implement [`Drop`],
/// and its [`Drop`] implementation will remove the listener from the [`Container`].
///
/// Thus, if you want the handle to live for as long as the [`Container`] itself,
/// it is instead recommended to create a non-idempotent capsule
/// (just call `register(effects::as_listener());`)
/// that acts as your listener. When you normally would call `container.listen()`,
/// instead call `container.read(my_nonidempotent_listener)` to initialize it.
pub struct ListenerHandle {
    id: CapsuleId,
    store: Weak<ContainerStore>,
}
impl Drop for ListenerHandle {
    fn drop(&mut self) {
        if let Some(store) = self.store.upgrade() {
            // NOTE: The node is guaranteed to be in the graph here since it is a listener.
            let orchestrator = SideEffectTxnOrchestrator(Weak::clone(&self.store));
            store.with_write_txn(orchestrator, |txn| txn.dispose_node(&self.id));
        }
    }
}

/// A list of capsules with cloneable data.
/// This is either a singular capsule, like `foo_capsule`,
/// or a tuple, like `(foo_capsule, bar_capsule)`.
pub trait CapsulesWithCloneRead {
    type Data;
    fn read(self, container: &Container) -> Self::Data;
}
macro_rules! generate_capsule_list_impl {
    ($($C:ident),+) => {
        paste::paste! {
            #[allow(non_snake_case, unused_parens)]
            impl<$($C: Capsule),*> CapsulesWithCloneRead for ($($C),*) where $($C::Data: Clone),* {
                type Data = ($($C::Data),*);
                fn read(self, container: &Container) -> Self::Data {
                    let ($([<i $C>]),*) = self;
                    if let ($(Some([<i $C>])),*) =
                        container.with_read_txn(|txn| ($(txn.try_read(&[<i $C>])),*)) {
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
#[derive(Default)]
struct ContainerStore {
    // NOTE: we store capsule data in an Arc here because it provides faster clones than a Box.
    // This is because the Arc will have low contention, as the clones are always behind a Mutex.
    data: concread::hashmap::HashMap<CapsuleId, Arc<dyn Any + Send + Sync>>,
    nodes: Mutex<std::collections::HashMap<CapsuleId, CapsuleManager>>,
    curr_side_effect_txn_modified_ids: ReentrantMutex<RefCell<Option<HashSet<CapsuleId>>>>,
}
impl ContainerStore {
    fn with_read_txn<'f, R>(&self, to_run: impl 'f + FnOnce(&ContainerReadTxn) -> R) -> R {
        let txn = ContainerReadTxn::new(self.data.read());
        to_run(&txn)
    }

    #[allow(clippy::significant_drop_tightening)] // false positive
    fn with_write_txn<'f, R>(
        &self,
        orchestrator: SideEffectTxnOrchestrator,
        to_run: impl 'f + FnOnce(&mut ContainerWriteTxn) -> R,
    ) -> R {
        let data = self.data.write();
        let nodes = &mut self.nodes.lock().expect("Mutex shouldn't fail to lock");
        let mut txn = ContainerWriteTxn::new(data, nodes, orchestrator);

        let return_val = to_run(&mut txn);

        // We must commit the txn to avoid leaving the data and nodes in an inconsistent state
        txn.data.commit();

        return_val
    }
}

type SideEffectStateMutation<'f> = Box<dyn 'f + FnOnce(&mut dyn Any)>;
type SideEffectStateMutationRunner = Arc<dyn Send + Sync + Fn(SideEffectStateMutation)>;
type SideEffectTxn<'f> = Box<dyn 'f + FnOnce()>;
type SideEffectTxnRunner = Arc<dyn Send + Sync + Fn(SideEffectTxn)>;

#[derive(Clone)]
struct SideEffectTxnOrchestrator(Weak<ContainerStore>);
impl SideEffectTxnOrchestrator {
    fn run_mutation(&self, id: CapsuleId, mutation: SideEffectStateMutation) {
        let Some(store) = self.0.upgrade() else {
            #[cfg(feature = "logging")]
            log::warn!(
                "Attempted to mutate side effect state after Container disposal on Capsule ({:?})",
                id
            );

            return;
        };

        #[cfg(feature = "logging")]
        log::debug!("Mutating side effect state in Capsule ({:?})", id);

        let orchestrator = self.clone();
        self.run_txn(Box::new(move || {
            // NOTE: The node is guaranteed to be in the graph here since it registers a side effect.
            store.with_write_txn(orchestrator, {
                let id = &id;
                move |txn| {
                    let effect = txn
                        .side_effect(id)
                        .get_mut()
                        .expect(concat!(
                            "The side effect must've been previously initialized ",
                            "in order to use the side effect state mutater"
                        ))
                        .as_mut();
                    mutation(effect);
                }
            });
            store
                .curr_side_effect_txn_modified_ids
                .lock()
                .borrow_mut()
                .as_mut()
                .expect("Called in a side effect txn, so txn should be Some")
                .insert(id);
        }));
    }

    fn run_txn<'f>(&self, txn: Box<dyn 'f + FnOnce()>) {
        let Some(store) = self.0.upgrade() else {
            #[cfg(feature = "logging")]
            log::warn!("Attempted to run a side effect txn after Container disposal");

            return;
        };
        let curr_txn_modified_ids = store.curr_side_effect_txn_modified_ids.lock();

        let is_root_txn = curr_txn_modified_ids.borrow().is_none();
        if is_root_txn {
            *curr_txn_modified_ids.borrow_mut() = Some(HashSet::new());
        }

        txn();

        if is_root_txn {
            let to_build = std::mem::take(&mut *curr_txn_modified_ids.borrow_mut())
                .expect("Ensured initialization above");
            store.with_write_txn(self.clone(), move |txn| {
                txn.build_capsules_or_panic(&to_build);
            });
        }

        drop(curr_txn_modified_ids); // ensure the lock is held until after the last store write txn
    }

    fn create_state_mutater_for_id(&self, id: CapsuleId) -> SideEffectStateMutationRunner {
        let orchestrator = self.clone();
        Arc::new(move |mutation| orchestrator.run_mutation(CapsuleId::clone(&id), mutation))
    }

    fn create_txn_runner(&self) -> SideEffectTxnRunner {
        let orchestrator = self.clone();
        Arc::new(move |txn| orchestrator.run_txn(txn))
    }
}

fn downcast_capsule_data<C: Capsule>(x: &impl Deref<Target = dyn Any + Send + Sync>) -> &C::Data {
    x.downcast_ref::<C::Data>()
        .expect("Types should be properly enforced due to generics")
}

const EXCLUSIVE_OWNER_MSG: &str =
    "Attempted to use a CapsuleManager field when someone else already had ownership";

// This struct is completely typeless in order to avoid *a lot* of dynamic dispatch
// that we used to have when dealing with the graph nodes.
// We avoid needing types by storing a fn pointer of a function that performs the actual build.
// A capsule's build is a capsule's only type-specific behavior!
// Note: we use Option over a few fields in CapsuleManager to enforce a safer ownership model
// (ownership of some of the CapsuleManager's fields must be taken during builds).
struct CapsuleManager {
    capsule: Option<Box<dyn Any + Send>>,
    side_effect: Option<OnceCell<Box<dyn Any + Send>>>,
    dependencies: HashSet<CapsuleId>,
    dependents: HashSet<CapsuleId>,
    build: fn(CapsuleId, &mut ContainerWriteTxn) -> bool,
}

impl CapsuleManager {
    fn new<C: Capsule>(capsule: C) -> Self {
        Self {
            capsule: Some(Box::new(capsule)),
            side_effect: Some(OnceCell::new()),
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            build: Self::build::<C>,
        }
    }

    /// Builds a capsule's new data and puts it into the txn, returning true when the data changes.
    fn build<C: Capsule>(id: CapsuleId, txn: &mut ContainerWriteTxn) -> bool {
        #[cfg(feature = "logging")]
        log::trace!("Building {} ({:?})", std::any::type_name::<C>(), id);

        let new_data = {
            let side_effect_state_mutater = txn
                .side_effect_txn_orchestrator
                .create_state_mutater_for_id(CapsuleId::clone(&id));
            let side_effect_txn_runner = txn.side_effect_txn_orchestrator.create_txn_runner();

            let (capsule, mut side_effect) = txn.take_capsule_and_side_effect(&id);
            let new_data = capsule
                .downcast_ref::<C>()
                .expect("Types should be properly enforced due to generics")
                .build(CapsuleHandle {
                    get: CapsuleReader::new(CapsuleId::clone(&id), txn),
                    register: SideEffectRegistrar::new(
                        &mut side_effect,
                        side_effect_state_mutater,
                        side_effect_txn_runner,
                    ),
                });
            txn.yield_capsule_and_side_effect(&id, capsule, side_effect);

            new_data
        };

        let did_change = txn
            .data
            .remove(&id)
            .as_ref()
            .map(downcast_capsule_data::<C>)
            .map_or(true, |old_data| !C::eq(old_data, &new_data));

        txn.data.insert(id, Arc::new(new_data));

        did_change
    }

    fn is_idempotent(&self) -> bool {
        self.side_effect
            .as_ref()
            .expect(EXCLUSIVE_OWNER_MSG)
            .get()
            .is_none()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::*;

    mod effects {
        use super::*;

        pub fn as_listener() -> impl for<'a> SideEffect<Api<'a> = ()> {}

        pub fn cloned_state<T: Clone + Send + 'static>(
            initial: T,
        ) -> impl for<'a> SideEffect<Api<'a> = (T, impl CData + Fn(T))> {
            move |register: SideEffectRegistrar| {
                let (state, rebuild, _) = register.raw(initial);
                let set_state = move |new_state| {
                    rebuild(Box::new(|state| *state = new_state));
                };
                (state.clone(), set_state)
            }
        }

        pub fn is_first_build() -> impl for<'a> SideEffect<Api<'a> = bool> {
            move |register: SideEffectRegistrar| {
                let (has_built_before, _, _) = register.raw(false);
                let is_first_build = !*has_built_before;
                *has_built_before = true;
                is_first_build
            }
        }

        pub fn rebuilder() -> impl for<'a> SideEffect<Api<'a> = impl CData + Fn()> {
            move |register: SideEffectRegistrar| {
                let ((), rebuild, _) = register.raw(());
                move || rebuild(Box::new(|()| {}))
            }
        }
    }

    #[test]
    const fn container_send_sync() {
        const fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<crate::Container>();
    }

    /// Check for some fundamental functionality with the classic count example
    #[test]
    fn basic_count() {
        fn count(_: CapsuleHandle) -> u8 {
            0
        }

        fn count_plus_one(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.as_ref(count) + 1
        }

        let container = Container::new();
        assert_eq!(
            (None, None),
            container.with_read_txn(|txn| (txn.try_read(&count), txn.try_read(&count_plus_one)))
        );
        assert_eq!(
            1,
            container.with_write_txn(|txn| txn.read_or_init(count_plus_one))
        );
        assert_eq!(
            0,
            container.with_read_txn(|txn| txn.try_read(&count).unwrap())
        );

        let container = Container::new();
        assert_eq!((0, 1), container.read((count, count_plus_one)));
    }

    mod state_updates {
        use super::*;

        fn stateful(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn(u8)) {
            register.register(effects::cloned_state(0))
        }

        fn dependent(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.as_ref(stateful).0 + 1
        }

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
    }

    #[test]
    fn multiple_side_effect() {
        fn foo(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (u8, u8, impl CData + Fn(u8), impl CData + Fn(u8)) {
            let ((s1, ss1), (s2, ss2)) =
                register.register((effects::cloned_state(0), effects::cloned_state(1)));
            (s1, s2, ss1, ss2)
        }

        let container = Container::new();

        let (s1, s2, set1, set2) = container.read(foo);
        assert_eq!(0, s1);
        assert_eq!(1, s2);

        set1(1);
        set2(2);
        let (s1, s2, _, _) = container.read(foo);
        assert_eq!(1, s1);
        assert_eq!(2, s2);
    }

    #[cfg(feature = "experimental-api")]
    #[test]
    fn get_and_register() {
        fn rebuildable(CapsuleHandle { register, .. }: CapsuleHandle) -> impl CData + Fn() {
            register(effects::rebuilder(), effects::as_listener()).0
        }

        fn build_counter(CapsuleHandle { mut get, register }: CapsuleHandle) -> usize {
            _ = get(rebuildable); // mark dep

            let is_first_build = register(effects::is_first_build());
            if is_first_build {
                1
            } else {
                get(build_counter) + 1
            }
        }

        let container = Container::new();
        assert_eq!(container.read(build_counter), 1);
        container.read(rebuildable)();
        assert_eq!(container.read(build_counter), 2);
        container.read(rebuildable)();
        assert_eq!(container.read(build_counter), 3);
    }

    #[test]
    fn listener_gets_updates() {
        use std::sync::{Arc, Mutex};

        fn stateful(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn(u8)) {
            register.register(effects::cloned_state(0))
        }

        let states = Arc::new(Mutex::new(Vec::new()));

        let effect_factory = || ();
        let listener = {
            let states = Arc::clone(&states);
            move |mut reader: CapsuleReader, ()| {
                let mut states = states.lock().unwrap();
                states.push(reader.as_ref(stateful).0);
            }
        };

        let container = Container::new();

        container.read(stateful).1(1);
        let handle = container.listen(effect_factory, listener.clone());
        container.read(stateful).1(2);
        container.read(stateful).1(3);

        drop(handle);
        container.read(stateful).1(4);

        container.read(stateful).1(5);
        let handle = container.listen(effect_factory, listener);
        container.read(stateful).1(6);
        container.read(stateful).1(7);

        drop(handle);
        container.read(stateful).1(8);

        let states = states.lock().unwrap();
        assert_eq!(*states, vec![1, 2, 3, 5, 6, 7]);
        drop(states);
    }

    #[test]
    fn listener_side_effects_update() {
        use std::sync::{Arc, Mutex};

        fn rebuildable(CapsuleHandle { register, .. }: CapsuleHandle) -> (impl CData + Fn()) {
            register.register(effects::rebuilder())
        }

        let states = Arc::new(Mutex::new(Vec::new()));

        let container = Container::new();
        let handle = {
            let states = Arc::clone(&states);
            container.listen(effects::is_first_build, move |mut get, is_first_build| {
                let _ = get.as_ref(rebuildable);
                states.lock().unwrap().push(is_first_build);
            })
        };

        container.read(rebuildable)();

        let states = states.lock().unwrap();
        assert_eq!(*states, vec![true, false]);
        drop(states);

        drop(handle);
    }

    #[test]
    fn listener_with_multiple_effects() {
        let container = Container::new();
        _ = container.listen(
            || (effects::is_first_build(), effects::is_first_build()),
            |_, (b1, b2)| {
                assert!(b1);
                assert!(b2);
            },
        );
    }

    #[test]
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn eq_check_skips_unneeded_rebuilds() {
        use std::{any::TypeId, collections::HashMap};

        static BUILDS: Mutex<OnceCell<HashMap<TypeId, u32>>> = Mutex::new(OnceCell::new());

        #[allow(clippy::needless_pass_by_value)]
        fn increment_build_count<C: Capsule>(_capsule: C) {
            let mut cell = BUILDS.lock().unwrap();
            cell.get_or_init(HashMap::new);
            let entry = cell.get_mut().unwrap().entry(TypeId::of::<C>());
            *entry.or_default() += 1;
            drop(cell);
        }
        #[allow(clippy::needless_pass_by_value)]
        fn get_build_count<C: Capsule>(_capsule: C) -> u32 {
            *BUILDS
                .lock()
                .unwrap()
                .get()
                .unwrap()
                .get(&TypeId::of::<C>())
                .unwrap()
        }

        macro_rules! define_cap {
            ($CapsuleName:ident, $body:expr) => {
                struct $CapsuleName;
                impl Capsule for $CapsuleName {
                    type Data = u32;
                    fn build(&self, CapsuleHandle { get, .. }: CapsuleHandle) -> Self::Data {
                        increment_build_count(Self);
                        #[allow(clippy::redundant_closure_call)]
                        $body(get)
                    }
                    fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                        old == new
                    }
                }
            };
        }

        fn stateful(CapsuleHandle { register, .. }: CapsuleHandle) -> (u32, impl CData + Fn(u32)) {
            increment_build_count(stateful);
            register.register(effects::cloned_state(0))
        }
        define_cap!(UnchangingIdempotentDep, |mut get: CapsuleReader| {
            _ = get.as_ref(stateful);
            0
        });
        define_cap!(UnchangingWatcher, |mut get: CapsuleReader| {
            *get.as_ref(UnchangingIdempotentDep)
        });
        define_cap!(ChangingIdempotentDep, |mut get: CapsuleReader| {
            get.as_ref(stateful).0
        });
        define_cap!(ChangingWatcher, |mut get: CapsuleReader| {
            *get.as_ref(ChangingIdempotentDep)
        });
        fn impure_sink(CapsuleHandle { mut get, register }: CapsuleHandle) {
            register.register(effects::as_listener());
            _ = get.as_ref(ChangingWatcher);
            _ = get.as_ref(UnchangingWatcher);
        }

        let container = Container::new();

        assert_eq!(container.read(UnchangingWatcher), 0);
        assert_eq!(container.read(ChangingWatcher), 0);
        assert_eq!(get_build_count(stateful), 1);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 1);
        assert_eq!(get_build_count(ChangingIdempotentDep), 1);
        assert_eq!(get_build_count(UnchangingWatcher), 1);
        assert_eq!(get_build_count(ChangingWatcher), 1);

        container.read(stateful).1(0);
        assert_eq!(get_build_count(stateful), 2);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 1);
        assert_eq!(get_build_count(ChangingIdempotentDep), 1);
        assert_eq!(get_build_count(UnchangingWatcher), 1);
        assert_eq!(get_build_count(ChangingWatcher), 1);

        assert_eq!(container.read(UnchangingWatcher), 0);
        assert_eq!(container.read(ChangingWatcher), 0);
        assert_eq!(get_build_count(stateful), 2);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 2);
        assert_eq!(get_build_count(ChangingIdempotentDep), 2);
        assert_eq!(get_build_count(UnchangingWatcher), 2);
        assert_eq!(get_build_count(ChangingWatcher), 2);

        container.read(stateful).1(1);
        assert_eq!(get_build_count(stateful), 3);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 2);
        assert_eq!(get_build_count(ChangingIdempotentDep), 2);
        assert_eq!(get_build_count(UnchangingWatcher), 2);
        assert_eq!(get_build_count(ChangingWatcher), 2);

        assert_eq!(container.read(UnchangingWatcher), 0);
        assert_eq!(container.read(ChangingWatcher), 1);
        assert_eq!(get_build_count(stateful), 3);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 3);
        assert_eq!(get_build_count(ChangingIdempotentDep), 3);
        assert_eq!(get_build_count(UnchangingWatcher), 3);
        assert_eq!(get_build_count(ChangingWatcher), 3);

        // Disable the idempotent gc
        container.read(impure_sink);

        container.read(stateful).1(2);
        assert_eq!(get_build_count(stateful), 4);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 4);
        assert_eq!(get_build_count(ChangingIdempotentDep), 4);
        assert_eq!(get_build_count(UnchangingWatcher), 3);
        assert_eq!(get_build_count(ChangingWatcher), 4);

        assert_eq!(container.read(UnchangingWatcher), 0);
        assert_eq!(container.read(ChangingWatcher), 2);
        assert_eq!(get_build_count(stateful), 4);
        assert_eq!(get_build_count(UnchangingIdempotentDep), 4);
        assert_eq!(get_build_count(ChangingIdempotentDep), 4);
        assert_eq!(get_build_count(UnchangingWatcher), 3);
        assert_eq!(get_build_count(ChangingWatcher), 4);
    }

    #[test]
    fn fib_dynamic_capsules() {
        struct FibCapsule(u8);
        impl Capsule for FibCapsule {
            type Data = u128;

            fn build(&self, CapsuleHandle { mut get, .. }: CapsuleHandle) -> Self::Data {
                let Self(n) = self;
                match n {
                    0 => 0,
                    1 => 1,
                    n => *get.as_ref(Self(n - 1)) + get.as_ref(Self(n - 2)),
                }
            }

            fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                old == new
            }

            fn key(&self) -> CapsuleKey {
                self.0.into()
            }
        }

        let container = Container::new();
        assert_eq!(container.read(FibCapsule(100)), 354_224_848_179_261_915_075);
    }

    #[test]
    fn dynamic_capsules_remain_isolated() {
        struct A(u8);
        impl Capsule for A {
            type Data = u8;

            fn build(&self, _: CapsuleHandle) -> Self::Data {
                self.0
            }

            fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                old == new
            }

            fn key(&self) -> CapsuleKey {
                self.0.into()
            }
        }
        struct B(u8);
        impl Capsule for B {
            type Data = u8;

            fn build(&self, _: CapsuleHandle) -> Self::Data {
                self.0 + 1
            }

            fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                old == new
            }

            fn key(&self) -> CapsuleKey {
                self.0.into()
            }
        }

        // A and B will have the same bytes in their keys, but should remain separate
        let container = Container::new();
        assert_eq!(container.read(A(0)), 0);
        assert_eq!(container.read(B(0)), 1);
    }

    #[test]
    fn dynamic_and_static_capsules() {
        fn stateful(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn(u8)) {
            register.register(effects::cloned_state(0))
        }
        struct Cell(u8);
        impl Capsule for Cell {
            type Data = u8;

            fn build(&self, CapsuleHandle { mut get, .. }: CapsuleHandle) -> Self::Data {
                self.0 + get.as_ref(stateful).0
            }

            fn eq(old: &Self::Data, new: &Self::Data) -> bool {
                old == new
            }

            fn key(&self) -> CapsuleKey {
                self.0.into()
            }
        }
        fn sink(CapsuleHandle { mut get, .. }: CapsuleHandle) -> (u8, u8) {
            (*get.as_ref(Cell(0)), *get.as_ref(Cell(1)))
        }

        let container = Container::new();
        assert_eq!(container.read(sink), (0, 1));
        container.read(stateful).1(1);
        assert_eq!(container.read(sink), (1, 2));
    }

    // We use a more sophisticated graph here for a more thorough test of all functionality
    //
    // -> A -> B -> C -> D
    //      \      / \
    //  H -> E -> F -> G
    //
    // C, D, E, G, H are idempotent. A, B, F are not.
    #[test]
    fn complex_dependency_graph() {
        fn stateful_a(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn(u8)) {
            register.register(effects::cloned_state(0))
        }

        fn a(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.as_ref(stateful_a).0
        }

        fn b(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            register.register(());
            get.as_ref(a) + 1
        }

        fn c(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            *get.as_ref(b) + get.as_ref(f)
        }

        fn d(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            *get.as_ref(c)
        }

        fn e(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            *get.as_ref(a) + get.as_ref(h)
        }

        fn f(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            register.register(());
            *get.as_ref(e)
        }

        fn g(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            *get.as_ref(c) + get.as_ref(f)
        }

        fn h(_: CapsuleHandle) -> u8 {
            1
        }

        let container = Container::new();
        let mut read_txn_counter = 0;

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(&stateful_a).is_none());
            assert_eq!(txn.try_read(&a), None);
            assert_eq!(txn.try_read(&b), None);
            assert_eq!(txn.try_read(&c), None);
            assert_eq!(txn.try_read(&d), None);
            assert_eq!(txn.try_read(&e), None);
            assert_eq!(txn.try_read(&f), None);
            assert_eq!(txn.try_read(&g), None);
            assert_eq!(txn.try_read(&h), None);
        });

        container.read((d, g));

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(&stateful_a).is_some());
            assert_eq!(txn.try_read(&a).unwrap(), 0);
            assert_eq!(txn.try_read(&b).unwrap(), 1);
            assert_eq!(txn.try_read(&c).unwrap(), 2);
            assert_eq!(txn.try_read(&d).unwrap(), 2);
            assert_eq!(txn.try_read(&e).unwrap(), 1);
            assert_eq!(txn.try_read(&f).unwrap(), 1);
            assert_eq!(txn.try_read(&g).unwrap(), 3);
            assert_eq!(txn.try_read(&h).unwrap(), 1);
        });

        container.read(stateful_a).1(10);

        container.with_read_txn(|txn| {
            read_txn_counter += 1;
            assert!(txn.try_read(&stateful_a).is_some());
            assert_eq!(txn.try_read(&a).unwrap(), 10);
            assert_eq!(txn.try_read(&b).unwrap(), 11);
            assert_eq!(txn.try_read(&c), None);
            assert_eq!(txn.try_read(&d), None);
            assert_eq!(txn.try_read(&e).unwrap(), 11);
            assert_eq!(txn.try_read(&f).unwrap(), 11);
            assert_eq!(txn.try_read(&g), None);
            assert_eq!(txn.try_read(&h).unwrap(), 1);
        });

        assert_eq!(read_txn_counter, 3);
    }

    mod side_effect_txns {
        use super::*;

        fn two_side_effects_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> ((u8, impl CData + Fn(u8)), (u8, impl CData + Fn(u8))) {
            register.register((effects::cloned_state(0), effects::cloned_state(1)))
        }

        fn another_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> (u8, impl CData + Fn(u8)) {
            register.register(effects::cloned_state(2))
        }

        fn batch_all_updates_action(
            CapsuleHandle { mut get, register }: CapsuleHandle,
        ) -> impl CData + Fn(u8) {
            let ((_, set_state1), (_, set_state2)) = get.as_ref(two_side_effects_capsule).clone();
            let (_, set_state3) = get.as_ref(another_capsule).clone();
            let ((), _, run_txn) = register.raw(());
            move |n| {
                run_txn(Box::new(|| {
                    set_state1(n);
                    set_state2(n);
                    set_state3(n);
                }));
            }
        }

        fn build_counter_capsule(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            let is_first_build = register.register(effects::is_first_build());

            _ = get.as_ref(two_side_effects_capsule);
            _ = get.as_ref(another_capsule);

            if is_first_build {
                1
            } else {
                get.as_ref(build_counter_capsule) + 1
            }
        }

        fn txn_runner_capsule(
            CapsuleHandle { register, .. }: CapsuleHandle,
        ) -> SideEffectTxnRunner {
            register.raw(()).2
        }

        #[test]
        fn one_capsule_with_multiple_side_effects() {
            let container = Container::new();

            assert_eq!(container.read(build_counter_capsule), 1);
            let ((s1, ss1), (s2, ss2)) = container.read(two_side_effects_capsule);
            assert_eq!(s1, 0);
            assert_eq!(s2, 1);

            container.read(txn_runner_capsule)(Box::new(move || {
                ss1(1);
                ss2(2);
            }));

            assert_eq!(container.read(build_counter_capsule), 2);
            let ((s1, _), (s2, _)) = container.read(two_side_effects_capsule);
            assert_eq!(s1, 1);
            assert_eq!(s2, 2);
        }

        #[test]
        fn multiple_capsules_with_one_side_effect_each() {
            let container = Container::new();

            assert_eq!(container.read(build_counter_capsule), 1);
            let ((s1, ss1), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, ss3) = container.read(another_capsule);
            assert_eq!(s1, 0);
            assert_eq!(s2, 1);
            assert_eq!(s3, 2);

            container.read(txn_runner_capsule)(Box::new(move || {
                ss1(123);
                ss3(123);
            }));

            assert_eq!(container.read(build_counter_capsule), 2);
            let ((s1, _), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, _) = container.read(another_capsule);
            assert_eq!(s1, 123);
            assert_eq!(s2, 1);
            assert_eq!(s3, 123);
        }

        #[test]
        fn multiple_capsules_with_multiple_side_effects() {
            let container = Container::new();

            assert_eq!(container.read(build_counter_capsule), 1);
            let ((s1, _), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, _) = container.read(another_capsule);
            assert_eq!(s1, 0);
            assert_eq!(s2, 1);
            assert_eq!(s3, 2);

            container.read(batch_all_updates_action)(123);

            assert_eq!(container.read(build_counter_capsule), 2);
            let ((s1, _), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, _) = container.read(another_capsule);
            assert_eq!(s1, 123);
            assert_eq!(s2, 123);
            assert_eq!(s3, 123);
        }

        #[test]
        fn nested_transactions() {
            let container = Container::new();

            assert_eq!(container.read(build_counter_capsule), 1);
            let ((s1, ss1), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, ss3) = container.read(another_capsule);
            assert_eq!(s1, 0);
            assert_eq!(s2, 1);
            assert_eq!(s3, 2);

            container.read(txn_runner_capsule)({
                Box::new(|| {
                    ss1(111);
                    container.read(batch_all_updates_action)(123);
                    ss3(111);
                })
            });

            assert_eq!(container.read(build_counter_capsule), 2);
            let ((s1, _), (s2, _)) = container.read(two_side_effects_capsule);
            let (s3, _) = container.read(another_capsule);
            assert_eq!(s1, 123);
            assert_eq!(s2, 123);
            assert_eq!(s3, 111);
        }
    }
}
