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

mod capsule_reader;
pub use capsule_reader::*;

mod side_effect_registrar;
pub use side_effect_registrar::*;

mod txn;
pub use txn::*;

// TODO convert side effects to this format
/*
fn state<T>(default: T) -> SideEffect<Api = (u8, Arc)> {
    move |register| {
        let (data, rebuilder) = register.state(default);
        (data, rebuilder)
    }
}

fn lazy_state<T, F: FnOnce() -> T>(init: F) -> SideEffect {
    move |register| {
        let (data, rebuilder) = register.state(LazyCell::new(init));
        (data.get_mut(), rebuilder)
    }
}

fn sync_persist<Read, Write, R, T>(read: Read, write: Write) {
    move |register| {
        let ((state, set_state), write) = register(lazy_state(read), value(Arc::new(write)));

        let write = Arc::clone(write);
        let persist = move |new_data| {
            let persist_result = write(new_data);
            set_state(persist_result);
        };

        (state, Arc::new(persist))
    }
}
*/
// TODO attempt to see if we can rewrite SideEffectRebuilder without Box<dyn> with new approach
//   (but keep the Box<dyn FnOnce(...)> inner part the same)
// TODO capsule macro
// TODO side effect macro to remove the `move |register| {}` boilerplate
// TODO aggressive garbage collection mode
//   (delete all created super pure capsules that aren't needed at end of a requested build)
// TODO listener function instead of exposed garbage collection.
//   container.listen(|get| do_something(get(some_capsule)))
//   returns a ListenerKeepAlive that removes listener once dropped
//   internally implemented as an "impure" capsule that is dropped when keep alive drops
//   what about the listener's dependencies? should they be trimmed if possible?
//   maybe go off container's aggressiveness setting

/// Capsules are blueprints for creating some immutable data
/// and do not actually contain any data themselves.
/// See the README for more.
///
/// Note: *Do not manually implement this trait yourself!*
/// It is an internal implementation detail that may be changed or removed in the future.
// - `Send` is required because `CapsuleManager` needs to store a copy of the capsule
// - `'static` is required to store a copy of the capsule, and for TypeId::of()
pub trait Capsule: Send + 'static {
    /// The type of data associated with this capsule.
    /// Capsule types must be `Clone + Send + Sync + 'static`.
    /// It is recommended to only put types with "cheap" clones in Capsules;
    /// think Copy types, small Vecs and other containers, basic data structures, and Arcs.
    /// If you are dealing with a bigger chunk of data, consider wrapping it in an Arc.
    /// Note: The `im` crate plays *very nicely* with rearch.
    // Associated type so that Capsule can only be implemented once for each concrete type
    type Data: CapsuleData;

    /// Builds the capsule's immutable data using a given snapshot of the data flow graph.
    /// (The snapshot, a `ContainerWriteTxn`, is abstracted away for you.)
    ///
    /// ABSOLUTELY DO NOT TRIGGER ANY REBUILDS WITHIN THIS FUNCTION!
    /// Doing so will result in a deadlock.
    fn build(&self, reader: CapsuleReader, effect: SideEffectRegistrar) -> Self::Data;
}

impl<T, F> Capsule for F
where
    T: CapsuleData,
    F: Fn(CapsuleReader, SideEffectRegistrar) -> T + Send + 'static,
{
    type Data = T;

    fn build(&self, reader: CapsuleReader, registrar: SideEffectRegistrar) -> Self::Data {
        self(reader, registrar)
    }
}

/// Represents the type of a capsule's data;
/// Capsules' data must be `Clone + Send + Sync + 'static`.
pub trait CapsuleData: Any + DynClone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> CapsuleData for T {}
dyn_clone::clone_trait_object!(CapsuleData);

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

    /// Construct this side effect's build api, given:
    /// - A mutable reference to the current state of this side effect (&mut self)
    /// - A mechanism to trigger rebuilds that can also update the state of this side effect
    fn api(&mut self, rebuild: Box<dyn SideEffectRebuilder<Self>>) -> Self::Api<'_>;
}

macro_rules! generate_tuple_side_effect_impl {
    ($($types:ident),*) => {
        impl<$($types: SideEffect),*> SideEffect for ($($types),*) {
            type Api<'a> = ($($types::Api<'a>),*);

            #[allow(unused_variables, clippy::unused_unit)]
            fn api(&mut self, rebuild: Box<dyn SideEffectRebuilder<Self>>) -> Self::Api<'_> {
                ($(
                    self.${index()}.api({
                        let rebuild = rebuild.clone();
                        let rebuild: Box<dyn SideEffectRebuilder<$types>> =
                            Box::new(move |mutation| rebuild(
                                Box::new(move |store| mutation(&mut store.${index()}))
                            ));
                        rebuild
                    })
                ),*)
            }
        }
    };
}

generate_tuple_side_effect_impl!(); // () is the no-op side effect
generate_tuple_side_effect_impl!(A, B);
generate_tuple_side_effect_impl!(A, B, C);
generate_tuple_side_effect_impl!(A, B, C, D);
generate_tuple_side_effect_impl!(A, B, C, D, E);
generate_tuple_side_effect_impl!(A, B, C, D, E, F);
generate_tuple_side_effect_impl!(A, B, C, D, E, F, G);
generate_tuple_side_effect_impl!(A, B, C, D, E, F, G, H);

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
        Self(Arc::new(ContainerStore::default()))
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
                .build(
                    CapsuleReader::new(id, txn),
                    SideEffectRegistrar::new(&mut side_effect, rebuilder),
                )
        };

        // Give manager ownership back over the fields we temporarily took
        let manager = txn.node_or_panic(id);
        manager.capsule = Some(capsule);
        manager.side_effect = Some(side_effect);

        txn.data.insert(id, Box::new(new_data));
    }

    fn is_super_pure(&self) -> bool {
        self.side_effect
            .as_ref()
            .expect(EXCLUSIVE_OWNER_MSG)
            .get()
            .is_none()
    }

    fn is_disposable(&self) -> bool {
        self.is_super_pure() && self.dependents.is_empty()
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
        fn count(_: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            0
        }

        fn count_plus_one(mut get: CapsuleReader, _: SideEffectRegistrar) -> u8 {
            get(count) + 1
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

    // We use a more sophisticated graph here for a more thorough test of all functionality
    //
    // -> A -> B -> C -> D
    //      \      / \
    //  H -> E -> F -> G
    //
    // C, D, E, G, H are super pure. A, B, F are not.
    #[test]
    fn complex_dependency_graph() {
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
