use std::collections::HashMap;

use rearch::{CData, Capsule, CapsuleHandle, CapsuleKey, Container};
use rearch_effects::{self as effects, LazyCloned};

// Imagine this capsule represents a view that depends on an element of the list_capsule,
// where the usize is the index of the list in a scrolling list view
struct ListElementViewCapsule(usize);
impl Capsule for ListElementViewCapsule {
    type Data = String;

    fn build(&self, CapsuleHandle { mut get, .. }: CapsuleHandle) -> Self::Data {
        get.as_ref(list_capsule).0[self.0].to_string()
    }

    fn eq(old: &Self::Data, new: &Self::Data) -> bool {
        old == new
    }

    fn key(&self) -> CapsuleKey {
        self.0.into()
    }
}

// Represents some list data source.
fn list_capsule(
    CapsuleHandle { register, .. }: CapsuleHandle,
) -> (im::Vector<u32>, impl CData + Fn(im::Vector<u32>)) {
    register.register(effects::state::<LazyCloned<_>>(im::Vector::new))
}

// Imagine this capsule represents the current indices of the list shown on screen,
// where a UI framework would modify the indices here to get the views to display.
// This may be done differently in an actual UI framework,
// but this just helps to show how ReArch is demand-driven.
fn watched_list_element_views_capsule(
    CapsuleHandle { mut get, register }: CapsuleHandle,
) -> (
    impl CData + Fn(usize), // Add an index to show
    impl CData + Fn(usize), // Remove an index to show
    HashMap<usize, String>, // Map of index to the current data at that index
) {
    let (indices, set_indices) =
        register.register(effects::state::<LazyCloned<_>>(im::HashSet::new));
    (
        {
            let indices = indices.clone();
            let set_indices = set_indices.clone();
            move |index| {
                let mut indices = indices.clone();
                indices.insert(index);
                set_indices(indices);
            }
        },
        {
            let indices = indices.clone();
            move |index| {
                let mut indices = indices.clone();
                indices.remove(&index);
                set_indices(indices);
            }
        },
        indices
            .into_iter()
            .map(|index| (index, get.as_ref(ListElementViewCapsule(index)).clone()))
            .collect(),
    )
}

// Normally, you should include the default state in the capsule itself.
// For the sake of this example,
// assume a user modified the list and that is where this data is coming from.
fn container_with_dummy_list_data() -> Container {
    let container = Container::new();
    let (_, set_list) = container.read(list_capsule);
    set_list((0..100).collect());
    container
}

fn main() {
    let container = container_with_dummy_list_data();

    // Say we have a couple of views currently shown on a hypothetical screen...
    for i in 10..12 {
        let (add_index, _, _) = container.read(watched_list_element_views_capsule);
        add_index(i);
    }

    // 10 and 11 should be available on this screen after coming into view.
    let (_, _, index_to_data) = container.read(watched_list_element_views_capsule);
    assert_eq!(index_to_data, {
        let mut map = HashMap::new();
        map.insert(10, "10".to_owned());
        map.insert(11, "11".to_owned());
        map
    });

    // And then let's say the user scrolls down (10 no longer visible, but 12 is)...
    let (_, remove_index, _) = container.read(watched_list_element_views_capsule);
    remove_index(10);
    let (add_index, _, _) = container.read(watched_list_element_views_capsule);
    add_index(12);

    // 11 and 12 should be available to show on the screen.
    let (_, _, index_to_data) = container.read(watched_list_element_views_capsule);
    assert_eq!(index_to_data, {
        let mut map = HashMap::new();
        map.insert(11, "11".to_owned());
        map.insert(12, "12".to_owned());
        map
    });

    // And any arbitrary changes to the list...
    let (mut list, set_list) = container.read(list_capsule);
    list[11] = 0;
    set_list(list);

    // Will only rebuild what is currently shown on screen
    // (and avoid rebuilding perhaps many views not shown on screen).
    // And this behavior can be customized based on the indices being watched!
    let (_, _, index_to_data) = container.read(watched_list_element_views_capsule);
    assert_eq!(index_to_data, {
        let mut map = HashMap::new();
        map.insert(11, "0".to_owned());
        map.insert(12, "12".to_owned());
        map
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_runs_correctly() {
        main();
    }
}
