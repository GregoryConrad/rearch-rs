#![allow(
    dead_code,
    unused_variables,
    clippy::needless_update,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unwrap_used
)]

use std::{any::Any, cell::OnceCell};

use rearch::SideEffectRegistrar;

fn sample_view(_: ViewHandle, _: ()) -> TerminatedView {
    view()
        .child(center, ())
        .inject(scoped_state, ())
        .child(padding, 16.0)
        .multichild(row, ())
        .children(vec![
            view_single(ez_text, "Hello World!".to_owned()),
            view_single(text, TextProps::default()),
            view()
                .inject(
                    text_props,
                    TextProps {
                        text: "Hello World!".to_owned(),
                        ..Default::default()
                    },
                )
                .end(injected_text, ()),
            view()
                .child(padding, 16.0)
                .multichild(column, (0, 0.0))
                .children(vec![
                    view_single(ez_text, "Hello World!".to_owned()),
                    view()
                        .inject(scoped_index_key::<usize>, 0)
                        .end(list_item, ()),
                ]),
        ])
}

// proc macro could produce sealed trait (to prevent impl elsewhere), new trait, and impl for View
// #[view] // which can add view.text() sort of gist as convenience, but not required
fn ez_text(_: ViewHandle, str: String) -> TerminatedView {
    view_single(
        text,
        TextProps {
            text: str,
            ..Default::default()
        },
    )
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
    view().keyed(index).end(
        text,
        TextProps {
            text: format!("{index}"),
            ..Default::default()
        },
    )
}

fn text_props(_: ViewHandle, props: TextProps) -> TextProps {
    props
}
fn injected_text(ViewHandle { mut context, .. }: ViewHandle, _: ()) -> TerminatedView {
    view_single(text, context.get(text_props).unwrap())
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

struct TerminatedView;
struct MultiChildView;

#[derive(Default)]
struct IntermediateView {
    curr_key_info: Option<(Key, KeysEqCheck)>,
}
impl IntermediateView {
    pub fn inject<F, T, U>(self, scope: F, props: T) -> Self
    where
        F: Fn(ViewHandle, T) -> U,
    {
        // invoke scope and put its data in descendant context
        self
    }

    pub fn keyed<T: PartialEq + 'static>(mut self, key: T) -> Self {
        let eq_check: fn(&Box<dyn Any>, &Box<dyn Any>) -> bool = keys_eq::<T>;
        self.curr_key_info = Some((Box::new(key), eq_check));
        self
    }

    pub fn child<F, T>(mut self, child: F, props: T) -> Self
    where
        F: Fn(ViewHandle, T) -> Self,
    {
        let key_info = std::mem::take(&mut self.curr_key_info);
        // append child to self
        self
    }

    pub fn multichild<F, T>(mut self, child: F, props: T) -> MultiChildView
    where
        F: Fn(ViewHandle, T) -> MultiChildView,
    {
        let key_info = std::mem::take(&mut self.curr_key_info);
        // append child to self
        MultiChildView
    }

    pub fn end<F, T>(mut self, child: F, props: T) -> TerminatedView
    where
        F: Fn(ViewHandle, T) -> TerminatedView,
    {
        let key_info = std::mem::take(&mut self.curr_key_info);
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

// rows, column, and others can use render/layout primitives provided in views themselves

#[derive(Clone, Default)]
struct TextProps {
    pub text: String,
}

fn text(_: ViewHandle, TextProps { text }: TextProps) -> TerminatedView {
    TerminatedView
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
