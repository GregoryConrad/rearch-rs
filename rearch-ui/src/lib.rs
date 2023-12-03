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
        .child(row, ())
        .children(vec![
            view().child(text, "Hello World!".to_owned()),
            view()
                .inject(text_style, TextStyle)
                .child(text, "Hello World Again!".to_owned()),
            view()
                .child(padding, 16.0)
                .child(column, (0, 0.0))
                .children(vec![
                    view().child(text, "A list item:".to_owned()),
                    view()
                        .inject(scoped_index_key::<usize>, 0)
                        .child(list_item, ()),
                ]),
        ])
}

// #[scoped] -> similar to #[view] above for convenience
fn scoped_state(_: ViewHandle, _: ()) -> u32 {
    0
}
fn scoped_index_key<T>(_: ViewHandle, index: T) -> T {
    index
}

fn list_item(ViewHandle { mut context, .. }: ViewHandle, _: ()) -> TerminatedView {
    let index = context.get(scoped_index_key::<usize>).unwrap();
    view().keyed(index).child(text, format!("{index}"))
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
type KeysEqCheck = fn(&Key, &Key) -> bool;
type InjectionBuilder = Box<dyn FnOnce(ViewHandle) -> Box<dyn Any>>;
type ChildBuilder = Box<dyn FnOnce(ViewHandle) -> IntermediateView>;

enum ViewLayer {
    Key {
        key: Key,
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

struct TerminatedView;
struct MultiChildView;
#[derive(Default)]
struct IntermediateView {
    layers: Vec<ViewLayer>,
}
impl IntermediateView {
    pub fn inject<Injection, Props, Data>(mut self, scope: Injection, props: Props) -> Self
    where
        Props: 'static,
        Data: 'static + Clone,
        Injection: 'static + Fn(ViewHandle, Props) -> Data,
    {
        self.layers.push(ViewLayer::Injection {
            injection_type: TypeId::of::<Injection>(),
            build_injection_data: Box::new(move |handle| Box::new(scope(handle, props))),
        });
        self
    }

    pub fn keyed<Key: PartialEq + 'static>(mut self, key: Key) -> Self {
        self.layers.push(ViewLayer::Key {
            key: Box::new(key),
            key_eq_check: keys_eq::<Key>,
        });
        self
    }

    pub fn child<Child, Props, Output>(self, child: Child, props: Props) -> Output
    where
        Output: FromIntermediateViewAndChild,
        Props: 'static,
        Child: 'static + Fn(ViewHandle, Props) -> Output,
    {
        Output::from(self, child, props)
    }
}
impl MultiChildView {
    fn children(self, children: Vec<TerminatedView>) -> TerminatedView {
        TerminatedView
    }
}

trait FromIntermediateViewAndChild {
    fn from<Child, Props>(intermediate_view: IntermediateView, child: Child, props: Props) -> Self
    where
        Props: 'static,
        Child: 'static + Fn(ViewHandle, Props) -> Self;
}
impl FromIntermediateViewAndChild for TerminatedView {
    fn from<Child, Props>(intermediate_view: IntermediateView, child: Child, props: Props) -> Self {
        Self
    }
}
impl FromIntermediateViewAndChild for MultiChildView {
    fn from<Child, Props>(intermediate_view: IntermediateView, child: Child, props: Props) -> Self {
        Self
    }
}
impl FromIntermediateViewAndChild for IntermediateView {
    fn from<Child, Props>(
        mut intermediate_view: IntermediateView,
        child: Child,
        props: Props,
    ) -> Self
    where
        Props: 'static,
        Child: 'static + Fn(ViewHandle, Props) -> Self,
    {
        intermediate_view.layers.push(ViewLayer::Child {
            child_type: TypeId::of::<Child>(),
            build_child: Box::new(move |handle| child(handle, props)),
        });
        intermediate_view
    }
}

fn view() -> IntermediateView {
    IntermediateView::default()
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
