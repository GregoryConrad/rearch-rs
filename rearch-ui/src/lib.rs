#![allow(
    dead_code,
    unused_variables,
    clippy::needless_update,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unwrap_used
)]

use std::{
    any::{Any, TypeId},
    cell::OnceCell,
};

use rearch::SideEffectRegistrar;

// proc macro could produce sealed trait (to prevent impl elsewhere), new trait, and impl for View
// #[view] // which can add view.sample_view() sort of deal as convenience, but not required
fn sample_view(_: ViewHandle, _: ()) -> TerminatedView {
    view()
        .child(center, ())
        .inject(scoped_state, ())
        .child(padding, 16.0)
        .multichild(row, ())
        .children(vec![
            view_single(text, "Hello World!".to_owned()),
            view()
                .inject(text_style, TextStyle)
                .end(text, "Hello World Again!".to_owned()),
            view()
                .child(padding, 16.0)
                .multichild(column, (0, 0.0))
                .children(vec![
                    view_single(text, "A list item:".to_owned()),
                    view()
                        .inject(scoped_index_key::<usize>, 0)
                        .end(list_item, ()),
                ]),
        ])
}

// #[scoped] -> similar to #[view] above for convenience
fn scoped_state(_: ViewHandle, _: ()) -> u32 {
    0
}
fn scoped_index_key<T: Clone>(_: ViewHandle, index: T) -> T {
    index
}

fn list_item(ViewHandle { mut context, .. }: ViewHandle, _: ()) -> TerminatedView {
    let index = context.get(scoped_index_key::<usize>).unwrap();
    view().keyed(index).end(text, format!("{index}"))
}

// THE FOLLOWING IS GLUE CODE FOR THE PROTOTYPE TO COMPILE

struct ViewCapsuleReader; // analogous to CapsuleReader but uses something like onNextUpdate in Dart
struct Context; // to support scoped state and other UI interactions (constraints)
struct ViewHandle<'side_effect> {
    pub get: ViewCapsuleReader,
    pub register: SideEffectRegistrar<'side_effect>, // all side effects from ReArch should just work here
    pub context: Context,
}

impl Context {
    fn get<F, Ret, Props>(&mut self, scope: F) -> Option<Ret>
    where
        F: Fn(ViewHandle, Props) -> Ret,
        Ret: Clone,
    {
        todo!()
        // Ret is Clone so it can be copied down amongst children in context
        // we can use im crate to make a HashMap<TypeId, Box<dyn Any + Clone> like in ReArch
        // to make all children efficiently access scoped state
    }
}

fn keys_eq<T: PartialEq + 'static>(old: &Key, new: &Key) -> bool {
    if let (Some(old), Some(new)) = (old.downcast_ref::<T>(), new.downcast_ref::<T>()) {
        old == new
    } else {
        false
    }
}

type Key = Box<dyn Any>;
type KeysEqCheck = fn(&Box<dyn Any>, &Box<dyn Any>) -> bool;
type InjectionBuilder = Box<dyn FnOnce(ViewHandle) -> Box<dyn Any>>;
type ChildBuilder = Box<dyn FnOnce(ViewHandle) -> IntermediateView>;

struct TerminatedView;
struct MultiChildView;

enum ViewLayer {
    Key {
        key: Key,
        key_type: TypeId,
        key_eq_check: KeysEqCheck,
    },
    Injection {
        injection_type: TypeId,
        build_injection_data: InjectionBuilder,
    },
    Child {
        child_type: TypeId,
        build_child: ChildBuilder,
    },
}

#[derive(Default)]
struct IntermediateView {
    layers: Vec<ViewLayer>,
}
impl IntermediateView {
    pub fn inject<F, T, U>(mut self, scope: F, props: T) -> Self
    where
        T: 'static,
        U: 'static,
        F: 'static + Fn(ViewHandle, T) -> U,
    {
        self.layers.push(ViewLayer::Injection {
            injection_type: TypeId::of::<F>(),
            build_injection_data: Box::new(move |handle| Box::new(scope(handle, props))),
        });
        self
    }

    pub fn keyed<T: PartialEq + 'static>(mut self, key: T) -> Self {
        self.layers.push(ViewLayer::Key {
            key_type: TypeId::of::<T>(),
            key: Box::new(key),
            key_eq_check: keys_eq::<T>,
        });
        self
    }

    pub fn child<F, T>(mut self, child: F, props: T) -> Self
    where
        T: 'static,
        F: 'static + Fn(ViewHandle, T) -> Self,
    {
        self.layers.push(ViewLayer::Child {
            child_type: TypeId::of::<F>(),
            build_child: Box::new(move |handle| child(handle, props)),
        });
        self
    }

    pub fn multichild<F, T>(self, child: F, props: T) -> MultiChildView
    where
        F: Fn(ViewHandle, T) -> MultiChildView,
    {
        // append child to self
        MultiChildView
    }

    pub fn end<F, T>(self, child: F, props: T) -> TerminatedView
    where
        F: Fn(ViewHandle, T) -> TerminatedView,
    {
        // append child to self
        TerminatedView
    }

    // Users could also make their own custom functions like child0, child1, etc.
    // that remove the need for passing tuples into child() instead of relying on the macro below
}
impl MultiChildView {
    fn children(self, children: Vec<TerminatedView>) -> TerminatedView {
        TerminatedView
    }
}

fn view() -> IntermediateView {
    IntermediateView::default()
}

// This one probably wouldn't need to change, as it is just sugar
fn view_single<F, T>(child: F, props: T) -> TerminatedView
where
    F: Fn(ViewHandle, T) -> TerminatedView,
{
    view().end(child, props)
}

// Enable type-based injection
fn data<T: Clone>(_: ViewHandle, data: T) -> T {
    data
}

// rows, column, and others can use render/layout primitives provided in views themselves

fn text(ViewHandle { mut context, .. }: ViewHandle, str: String) -> TerminatedView {
    let style = context.get(text_style);
    TerminatedView
}
#[derive(Clone, Default)]
struct TextStyle;
fn text_style(ViewHandle { mut context, .. }: ViewHandle, style: TextStyle) -> TextStyle {
    let parent = context.get(text_style);
    // merge style with one from parent
    style
}

fn padding(_: ViewHandle, padding: f64) -> IntermediateView {
    view()
}
fn row(_: ViewHandle, _: ()) -> MultiChildView {
    MultiChildView
}
fn column(_: ViewHandle, (alignment, padding): (i32, f64)) -> MultiChildView {
    MultiChildView
}
fn center(_: ViewHandle, _: ()) -> IntermediateView {
    view()
}

struct ViewNode {
    key: Box<dyn Any>,
    side_effect: OnceCell<Box<dyn Any + Send>>,
    constraints: (),
}
