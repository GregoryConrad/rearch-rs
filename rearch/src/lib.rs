#![feature(trait_upcasting)]
#![cfg_attr(feature = "better-api", feature(unboxed_closures, fn_traits))]

use dyn_clone::DynClone;
use std::{
    any::{Any, TypeId},
    cell::OnceCell,
    collections::HashSet,
    sync::{Arc, Mutex, Weak},
};

pub mod side_effects;

mod capsule_reader;
pub use capsule_reader::*;

mod side_effect_registrar;
pub use side_effect_registrar::*;

mod txn;
pub use txn::*;

/// Capsules are blueprints for creating some immutable data
/// and do not actually contain any data themselves.
/// See the documentation for more.
///
// TODO(GregoryConrad): remove the following doc comment when this trait stabilizes.
/// *DO NOT MANUALLY IMPLEMENT THIS TRAIT YOURSELF!*
/// It is an internal implementation detail that will likely be changed or removed in the future.
// - `Send` is required because `CapsuleManager` needs to store a copy of the capsule
// - `'static` is required to store a copy of the capsule, and for TypeId::of()
pub trait Capsule: Send + 'static {
    /// The type of data associated with this capsule.
    /// Capsule types must be `Clone + Send + Sync + 'static` (see [`CapsuleData`]).
    /// It is recommended to only put types with "cheap" clones in Capsules;
    /// think Copy types, small Vecs and other containers, basic data structures, and Arcs.
    /// If you are dealing with a bigger chunk of data, consider wrapping it in an [`Arc`].
    /// Note: The `im` crate plays *very nicely* with rearch.
    // Associated type so that Capsule can only be implemented once for each concrete type
    type Data: CapsuleData;

    /// Builds the capsule's immutable data using a given snapshot of the data flow graph.
    /// (The snapshot, a `ContainerWriteTxn`, is abstracted away for you via [`CapsuleHandle`].)
    ///
    /// ABSOLUTELY DO NOT TRIGGER ANY REBUILDS WITHIN THIS FUNCTION!
    /// Doing so will result in a deadlock.
    fn build(&self, handle: CapsuleHandle) -> Self::Data;

    // TODO(GregoryConrad): the following eq method to prevent propagation when possible
    // fn eq(old: &Self::Data, new: &Self::Data) -> bool;
}
impl<T, F> Capsule for F
where
    T: CapsuleData,
    F: Fn(CapsuleHandle) -> T + Send + 'static,
{
    type Data = T;

    fn build(&self, handle: CapsuleHandle) -> Self::Data {
        self(handle)
    }
}

/// Represents the type of a capsule's data;
/// Capsules' data must be `Clone + Send + Sync + 'static`.
/// You seldom need to reference this in your application's code;
/// you are probably looking for [`CData`] instead.
pub trait CapsuleData: Any + DynClone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CapsuleData for T {}
dyn_clone::clone_trait_object!(CapsuleData);

/// Shorthand for `Clone + Send + Sync + 'static`,
/// which makes returning `impl Trait` far easier from capsules,
/// where `Trait` is often a `Fn(Foo) -> Bar`.
pub trait CData: Clone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CData for T {}

/// The handle given to [`Capsule`]s in order to [`Capsule::build`] their [`Capsule::Data`].
/// See [`CapsuleReader`] and [`SideEffectRegistrar`] for more.
pub struct CapsuleHandle<'txn_scope, 'txn_total, 'build> {
    pub get: CapsuleReader<'txn_scope, 'txn_total>,
    pub register: SideEffectRegistrar<'build>,
}

/// Represents a side effect that can be utilized within the build function.
/// The key observation about side effects is that they form a tree, where each side effect:
/// - Has its own private state (including composing other side effects together)
/// - Presents some api to the build method, probably including a way to rebuild & update its state
///
/// *DO NOT MANUALLY IMPLEMENT THIS TRAIT YOURSELF!*
/// It is an internal implementation detail that could be changed or removed in the future.
pub trait SideEffect<'a> {
    /// The type exposed in the capsule build function when this side effect is registered;
    /// in other words, this is the api exposed by the side effect.
    ///
    /// Often, a side effect's api is a tuple, containing values like:
    /// - Data and/or state in this side effect
    /// - Function callbacks (perhaps to trigger a rebuild and/or update the side effect state)
    /// - Anything else imaginable!
    type Api;

    /// Construct this side effect's `Api` via the given [`SideEffectRegistrar`].
    fn build(self, registrar: SideEffectRegistrar<'a>) -> Self::Api;
}
impl<'a, T, F: FnOnce(SideEffectRegistrar<'a>) -> T> SideEffect<'a> for F {
    type Api = T;
    fn build(self, registrar: SideEffectRegistrar<'a>) -> Self::Api {
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

    /// Runs the supplied callback with a `ContainerReadTxn` that allows you to read
    /// the current data in the container.
    ///
    /// You almost never want to use this function directly!
    /// Instead, use `read()` which wraps around `with_read_txn` and `with_write_txn`
    /// and ensures a consistent read amongst all capsules without extra effort.
    pub fn with_read_txn<R>(&self, to_run: impl FnOnce(&ContainerReadTxn) -> R) -> R {
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
    pub fn read<CL: CapsuleList>(&self, capsules: CL) -> CL::Data {
        capsules.read(self)
    }

    /* TODO(GregoryConrad): uncomment this listener section once we have side effects figured out
    /// Provides a mechanism to *temporarily* listen to changes in some capsule(s).
    /// The provided listener is called once at the time of the listener's registration,
    /// and then once again everytime a dependency changes.
    ///
    /// Returns a `ListenerHandle`, which doesn't do anything other than implement Drop,
    /// and its Drop implementation will remove `listener` from the Container.
    ///
    /// Thus, if you want the handle to live for as long as the Container itself,
    /// it is instead recommended to create a non-idempotent capsule
    /// (use the [`side_effects::as_listener()`] side effect)
    /// that acts as your listener. When you normally would call `Container::listen()`,
    /// instead call `container.read(my_non_idempotent_listener)` to initialize it.
    ///
    /// # Panics
    /// Panics if you attempt to register the same listener twice,
    /// before the first `ListenerHandle` is dropped.
    #[must_use]
    pub fn listen<ListenerEffect, EffectFactory, Listener>(
        &self,
        effect_factory: EffectFactory,
        listener: Listener,
    ) -> ListenerHandle
    where
        ListenerEffect: for<'a> SideEffect<'a>,
        EffectFactory: Fn() -> ListenerEffect + Send + Clone + 'static,
        Listener: Fn(CapsuleReader, <ListenerEffect as SideEffect>::Api) + Send + 'static,
    {
        // We make a temporary non-idempotent capsule for the listener so that
        // it doesn't get disposed by the idempotent gc
        let tmp_capsule = move |CapsuleHandle { get, register }: CapsuleHandle| {
            let effect_factory = effect_factory.clone();
            let effect = effect_factory();
            let effect_state = register.register(effect);
            listener(get, effect_state);
        };
        let id = tmp_capsule.type_id();

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
    */
}

/// Represents a handle onto a particular listener, as created with `Container::listen()`.
///
/// This struct doesn't do anything other than implement [`Drop`],
/// and its [`Drop`] implementation will remove the listener from the Container.
///
/// Thus, if you want the handle to live for as long as the Container itself,
/// it is instead recommended to create a non-idempotent capsule
/// (just call `register(as_listener());`)
/// that acts as your listener. When you normally would call `container.listen()`,
/// instead call `container.read(my_nonidempotent_listener)` to initialize it.
pub struct ListenerHandle {
    id: TypeId,
    store: Weak<ContainerStore>,
}
impl Drop for ListenerHandle {
    fn drop(&mut self) {
        if let Some(store) = self.store.upgrade() {
            // Note: The node is guaranteed to be in the graph here since it is a listener.
            let rebuilder = CapsuleRebuilder(Weak::clone(&self.store));
            store.with_write_txn(rebuilder, |txn| txn.dispose_single_node(self.id));
        }
    }
}

/// A list of capsules.
/// This is either a singular capsule, like `count`, or a tuple, like `(foo, bar)`.
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
/// Keys for both are simply the `TypeId` of capsules, like `TypeId::of::<SomeCapsule>()`.
#[derive(Default)]
struct ContainerStore {
    data: concread::hashmap::HashMap<TypeId, Box<dyn CapsuleData>>,
    nodes: Mutex<std::collections::HashMap<TypeId, CapsuleManager>>,
}
impl ContainerStore {
    fn with_read_txn<R>(&self, to_run: impl FnOnce(&ContainerReadTxn) -> R) -> R {
        let txn = ContainerReadTxn::new(self.data.read());
        to_run(&txn)
    }

    fn with_write_txn<R>(
        &self,
        rebuilder: CapsuleRebuilder,
        to_run: impl FnOnce(&mut ContainerWriteTxn) -> R,
    ) -> R {
        let data = self.data.write();
        let nodes = &mut self.nodes.lock().expect("Mutex shouldn't fail to lock");
        let mut txn = ContainerWriteTxn::new(data, nodes, rebuilder);

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
            // and using the side effect handle prevents the idempotent gc.)
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
    dependencies: HashSet<TypeId>,
    dependents: HashSet<TypeId>,
    build: fn(&mut ContainerWriteTxn),
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

    fn build<C: Capsule>(txn: &mut ContainerWriteTxn) {
        let id = TypeId::of::<C>();

        #[cfg(feature = "logging")]
        log::trace!("Building {} ({:?})", std::any::type_name::<C>(), id);

        // Take ownership over a few fields from the manager
        let manager = txn.node_or_panic(id);
        let capsule = std::mem::take(&mut manager.capsule).expect(EXCLUSIVE_OWNER_MSG);
        let mut side_effect = std::mem::take(&mut manager.side_effect).expect(EXCLUSIVE_OWNER_MSG);

        let new_data = {
            let rebuilder = {
                let rebuilder = txn.rebuilder.clone();
                Box::new(move |mutation: Box<dyn FnOnce(&mut Box<_>)>| {
                    rebuilder.rebuild(id, |manager| {
                        let effect = manager.side_effect.as_mut().expect(EXCLUSIVE_OWNER_MSG);
                        let effect = effect.get_mut().expect(concat!(
                            "The side effect must've been previously initialized ",
                            "in order to use the rebuilder"
                        ));
                        mutation(effect);
                    });
                })
            };

            capsule
                .downcast_ref::<C>()
                .expect("Types should be properly enforced due to generics")
                .build(CapsuleHandle {
                    get: CapsuleReader::new(id, txn),
                    register: SideEffectRegistrar::new(&mut side_effect, rebuilder),
                })
        };

        // Give manager ownership back over the fields we temporarily took
        let manager = txn.node_or_panic(id);
        manager.capsule = Some(capsule);
        manager.side_effect = Some(side_effect);

        txn.data.insert(id, Box::new(new_data));
    }

    fn is_idempotent(&self) -> bool {
        self.side_effect
            .as_ref()
            .expect(EXCLUSIVE_OWNER_MSG)
            .get()
            .is_none()
    }

    fn is_disposable(&self) -> bool {
        self.is_idempotent() && self.dependents.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::missing_const_for_fn)]
mod tests {
    use crate::*;

    /// Check for Container: Send + Sync
    #[allow(unused)]
    mod container_thread_safe {
        struct SyncSendCheck<T: Send + Sync>(T);
        const fn foo(bar: &SyncSendCheck<crate::Container>) {}
    }

    /// Check for some fundamental functionality with the classic count example
    #[test]
    fn basic_count() {
        fn count(_: CapsuleHandle) -> u8 {
            0
        }

        fn count_plus_one(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(count) + 1
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
        use crate::*;

        fn stateful(CapsuleHandle { register, .. }: CapsuleHandle) -> (u8, impl CData + Fn(u8)) {
            let (state, set_state) = register.register(side_effects::state(0));
            (*state, set_state)
        }

        fn dependent(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(stateful).0 + 1
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
                register.register((side_effects::state(0), side_effects::state(1)));
            (*s1, *s2, ss1, ss2)
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

    /*
        #[test]
        fn listener_gets_updates() {
            use std::sync::{Arc, Mutex};

            fn stateful(
                CapsuleHandle { register, .. }: CapsuleHandle,
            ) -> (u8, impl CData + Fn(u8)) {
                let (state, set_state) = register.register(side_effects::state(0));
                (*state, set_state)
            }

            let states = Arc::new(Mutex::new(Vec::new()));

            let effect_factory = || ();
            let listener = {
                let states = states.clone();
                move |mut reader: CapsuleReader, _| {
                    let mut states = states.lock().unwrap();
                    states.push(reader.get(stateful).0);
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
        }

        #[test]
        fn listener_side_effects_update() {
            use std::sync::{Arc, Mutex};

            fn rebuildable(
                CapsuleHandle { register, .. }: CapsuleHandle,
            ) -> (impl CData + Fn()) {
                register.register(side_effects::rebuilder())
            }

            let states = Arc::new(Mutex::new(Vec::new()));

            let container = Container::new();
            fn thing() -> impl SideEffect<'a, Api = bool> {
                side_effects::is_first_build()
            }
            let handle = container.listen(thing, |mut get, is_first_build| {
                get.get(rebuildable);
                states.clone().lock().unwrap().push(is_first_build);
            });

            container.read(rebuildable)();

            let states = states.lock().unwrap();
            assert_eq!(*states, vec![true, false])
        }
    */

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
            let (state, set_state) = register.register(side_effects::state(0));
            (*state, set_state)
        }

        fn a(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(stateful_a).0
        }

        fn b(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            register.register(());
            get.get(a) + 1
        }

        fn c(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(b) + get.get(f)
        }

        fn d(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(c)
        }

        fn e(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(a) + get.get(h)
        }

        fn f(CapsuleHandle { mut get, register }: CapsuleHandle) -> u8 {
            register.register(());
            get.get(e)
        }

        fn g(CapsuleHandle { mut get, .. }: CapsuleHandle) -> u8 {
            get.get(c) + get.get(f)
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
}
