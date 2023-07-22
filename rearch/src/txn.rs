use concread::hashmap::{HashMapReadTxn, HashMapWriteTxn};
use std::{
    any::{Any, TypeId},
    collections::HashSet,
};

use crate::{Capsule, CapsuleData, CapsuleManager, CapsuleRebuilder};

#[allow(clippy::module_name_repetitions)]
pub struct ContainerReadTxn<'a> {
    data: HashMapReadTxn<'a, TypeId, Box<dyn CapsuleData>>,
}

impl<'a> ContainerReadTxn<'a> {
    pub(crate) fn new(data: HashMapReadTxn<'a, TypeId, Box<dyn CapsuleData>>) -> Self {
        Self { data }
    }
}

impl ContainerReadTxn<'_> {
    #[must_use]
    pub fn try_read<C: Capsule>(&self, _capsule: &C) -> Option<C::Data> {
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

#[allow(clippy::module_name_repetitions)]
pub struct ContainerWriteTxn<'a> {
    pub(crate) rebuilder: CapsuleRebuilder,
    pub(crate) data: HashMapWriteTxn<'a, TypeId, Box<dyn CapsuleData>>,
    nodes: &'a mut std::collections::HashMap<TypeId, CapsuleManager>,
}

impl<'a> ContainerWriteTxn<'a> {
    pub(crate) fn new(
        data: HashMapWriteTxn<'a, TypeId, Box<dyn CapsuleData>>,
        nodes: &'a mut std::collections::HashMap<TypeId, CapsuleManager>,
        rebuilder: CapsuleRebuilder,
    ) -> Self {
        Self {
            rebuilder,
            data,
            nodes,
        }
    }
}

impl ContainerWriteTxn<'_> {
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

    #[must_use]
    pub fn try_read<C: Capsule>(&self, _capsule: &C) -> Option<C::Data> {
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

    /// Triggers a first build or rebuild for the supplied capsule
    fn build_capsule<C: Capsule>(&mut self, capsule: C) {
        let id = TypeId::of::<C>();

        if let std::collections::hash_map::Entry::Vacant(e) = self.nodes.entry(id) {
            e.insert(CapsuleManager::new(capsule));
        }

        self.build_capsule_or_panic(id);
    }

    /// Forcefully builds the capsule with the supplied id. Panics if node is not in the graph
    pub(crate) fn build_capsule_or_panic(&mut self, id: TypeId) {
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
    pub(crate) fn node_or_panic(&mut self, id: TypeId) -> &mut CapsuleManager {
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

    /// Forcefully disposes only the requested node, cleaning up the node's direct dependencies.
    /// Panics if the node is not in the graph.
    pub(crate) fn dispose_single_node(&mut self, id: TypeId) {
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

    /// Creates the start node's dependent subgraph build order, including start, *as a stack*.
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

    /// Helper function that given a `build_order`, garbage collects all idempotent nodes
    /// that have no dependents (i.e., they are entirely disposable)
    /// and returns the new build order without the (now garbage collected) idempotent nodes.
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
