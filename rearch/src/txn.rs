use concread::hashmap::{HashMapReadTxn, HashMapWriteTxn};
use std::{any::Any, cell::OnceCell, collections::HashSet, sync::Arc};

use crate::{Capsule, CapsuleManager, CapsuleRebuilder, CreateId, Id, EXCLUSIVE_OWNER_MSG};

#[allow(clippy::module_name_repetitions)] // re-exported at crate level (not in module)
pub struct ContainerReadTxn<'a> {
    data: HashMapReadTxn<'a, Id, Arc<dyn Any + Send + Sync>>,
}

impl<'a> ContainerReadTxn<'a> {
    pub(crate) fn new(data: HashMapReadTxn<'a, Id, Arc<dyn Any + Send + Sync>>) -> Self {
        Self { data }
    }
}

impl ContainerReadTxn<'_> {
    #[must_use]
    pub fn try_read<C: Capsule>(&self, capsule: &C) -> Option<C::Data>
    where
        C::Data: Clone,
    {
        self.try_read_ref(capsule).map(Clone::clone)
    }

    #[must_use]
    pub fn try_read_ref<C: Capsule>(&self, capsule: &C) -> Option<&C::Data> {
        self.data
            .get(&capsule.id())
            .map(crate::downcast_capsule_data::<C>)
    }
}

#[allow(clippy::module_name_repetitions)] // re-exported at crate level (not in module)
pub struct ContainerWriteTxn<'a> {
    pub(crate) rebuilder: CapsuleRebuilder,
    pub(crate) data: HashMapWriteTxn<'a, Id, Arc<dyn Any + Send + Sync>>,
    nodes: &'a mut std::collections::HashMap<Id, CapsuleManager>,
}

impl<'a> ContainerWriteTxn<'a> {
    pub(crate) fn new(
        data: HashMapWriteTxn<'a, Id, Arc<dyn Any + Send + Sync>>,
        nodes: &'a mut std::collections::HashMap<Id, CapsuleManager>,
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
    pub fn read_or_init<C: Capsule>(&mut self, capsule: C) -> C::Data
    where
        C::Data: Clone,
    {
        self.read_or_init_ref(capsule).clone()
    }

    #[allow(clippy::missing_panics_doc)] // false positive
    pub fn read_or_init_ref<C: Capsule>(&mut self, capsule: C) -> &C::Data {
        let id = capsule.id();
        self.ensure_initialized(capsule);
        self.try_read_ref_raw::<C>(&id)
            .expect("Ensured capsule was initialized above")
    }

    #[must_use]
    pub fn try_read<C: Capsule>(&self, capsule: &C) -> Option<C::Data>
    where
        C::Data: Clone,
    {
        self.try_read_ref::<C>(capsule).map(Clone::clone)
    }

    #[must_use]
    pub fn try_read_ref<C: Capsule>(&self, capsule: &C) -> Option<&C::Data> {
        self.try_read_ref_raw::<C>(&capsule.id())
    }

    pub(crate) fn try_read_ref_raw<C: Capsule>(&self, id: &Id) -> Option<&C::Data> {
        self.data.get(id).map(crate::downcast_capsule_data::<C>)
    }

    pub(crate) fn ensure_initialized<C: Capsule>(&mut self, capsule: C) {
        let id = capsule.id();
        if !self.data.contains_key(&id) {
            #[cfg(feature = "logging")]
            log::debug!("Initializing {} ({:?})", std::any::type_name::<C>(), id);

            self.build_capsule(capsule);
        }
    }

    /// Forcefully disposes only the requested node, cleaning up the node's direct dependencies.
    /// Panics if the node or one of its dependencies is not in the graph.
    pub(crate) fn dispose_node(&mut self, id: &Id) {
        self.data.remove(id);
        self.nodes
            .remove(id)
            .expect("Node should be in graph")
            .dependencies
            .iter()
            .for_each(|dep| {
                self.node_or_panic(dep).dependents.remove(id);
            });
    }

    pub(crate) fn add_dependency_relationship(&mut self, dependency: Id, dependent: &Id) {
        self.node_or_panic(&dependency)
            .dependents
            .insert(Arc::clone(dependent));
        self.node_or_panic(dependent)
            .dependencies
            .insert(dependency);
    }

    /// Forcefully builds the capsule with the supplied id. Panics if node is not in the graph
    pub(crate) fn build_capsule_or_panic(&mut self, id: Id) {
        let self_changed = self.build_single_node(&id);
        if !self_changed {
            return;
        }

        let mut build_order_stack = self.create_build_order_stack(Arc::clone(&id));
        build_order_stack.pop(); // we built id already above, and id is the head of the stack
        let disposable_nodes = self.get_disposable_nodes_from_build_order_stack(&build_order_stack);

        let mut changed_nodes = HashSet::new();
        changed_nodes.insert(id);

        for curr_id in build_order_stack.into_iter().rev() {
            let node = self.node_or_panic(&curr_id);

            let have_deps_changed = node
                .dependencies
                .iter()
                .any(|dep| changed_nodes.contains(dep));
            if !have_deps_changed {
                continue;
            }

            if disposable_nodes.contains(&curr_id) {
                // Note: dependency/dependent relationships will be ok after this,
                // since we are disposing all dependents in the build order,
                // because we are adding this node to changedNodes
                self.dispose_single_node(&curr_id);
                changed_nodes.insert(curr_id);
            } else {
                let did_node_change = self.build_single_node(&curr_id);
                if did_node_change {
                    changed_nodes.insert(curr_id);
                }
            }
        }
    }

    pub(crate) fn take_capsule_and_side_effect(
        &mut self,
        id: &Id,
    ) -> (Box<dyn Any + Send>, OnceCell<Box<dyn Any + Send>>) {
        let node = self.node_or_panic(id);
        let capsule = std::mem::take(&mut node.capsule).expect(EXCLUSIVE_OWNER_MSG);
        let side_effect = std::mem::take(&mut node.side_effect).expect(EXCLUSIVE_OWNER_MSG);
        (capsule, side_effect)
    }

    pub(crate) fn yield_capsule_and_side_effect(
        &mut self,
        id: &Id,
        capsule: Box<dyn Any + Send>,
        side_effect: OnceCell<Box<dyn Any + Send>>,
    ) {
        let node = self.node_or_panic(id);
        assert!(
            node.capsule.is_none() && node.side_effect.is_none(),
            "Manager had ownership over a capsule and side effect when ownership was yielded back",
        );
        node.capsule = Some(capsule);
        node.side_effect = Some(side_effect);
    }

    pub(crate) fn side_effect(&mut self, id: &Id) -> &mut OnceCell<Box<dyn Any + Send>> {
        self.node_or_panic(id)
            .side_effect
            .as_mut()
            .expect(EXCLUSIVE_OWNER_MSG)
    }

    /// Triggers a first build or rebuild for the supplied capsule
    fn build_capsule<C: Capsule>(&mut self, capsule: C) {
        let id = capsule.id();

        if let std::collections::hash_map::Entry::Vacant(e) = self.nodes.entry(Arc::clone(&id)) {
            e.insert(CapsuleManager::new(capsule));
        }

        self.build_capsule_or_panic(id);
    }

    /// Gets the requested node if it is in the graph
    fn node(&mut self, id: &Id) -> Option<&mut CapsuleManager> {
        self.nodes.get_mut(id)
    }

    /// Gets the requested node or panics if it is not in the graph
    fn node_or_panic(&mut self, id: &Id) -> &mut CapsuleManager {
        self.node(id)
            .expect("Requested node should be in the graph")
    }

    /// Builds only the requested node. Panics if the node is not in the graph
    fn build_single_node(&mut self, id: &Id) -> bool {
        // Remove old dependency info since it may change on this build
        // We use std::mem::take below to prevent needing a clone on the existing dependencies
        let node = self.node_or_panic(id);
        let old_deps = std::mem::take(&mut node.dependencies);
        for dep in old_deps {
            self.node_or_panic(&dep).dependents.remove(id);
        }

        // Trigger the build (which also populates its new dependencies in self)
        (self.node_or_panic(id).build)(Arc::clone(id), self)
    }

    /// Disposes just the supplied node, and *attempts* to clean up the node's direct dependencies.
    /// *This is meant to be a helper only for [`build_capsule_or_panic`]*,
    /// as an idempotent node getting disposed in that method may have dependencies that
    /// were already disposed from the graph.
    /// In all other cases, [`dispose_node`] is likely the proper method to use.
    fn dispose_single_node(&mut self, id: &Id) {
        self.data.remove(id);
        self.nodes
            .remove(id)
            .expect("Node should be in graph")
            .dependencies
            .iter()
            .for_each(|dep| {
                if let Some(node) = self.node(dep) {
                    node.dependents.remove(id);
                }
            });
    }

    /// Creates the start node's dependent subgraph build order, including start, *as a stack*.
    /// Thus, proper iteration order is done by popping off of the stack (in reverse order)!
    fn create_build_order_stack(&mut self, start: Id) -> Vec<Id> {
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
                visited.insert(Arc::clone(&node));
                to_visit_stack.push((true, Arc::clone(&node))); // mark node to be visited later
                self.node_or_panic(&node)
                    .dependents
                    .iter()
                    .filter(|dep| !visited.contains(*dep))
                    .cloned()
                    .for_each(|dep| to_visit_stack.push((false, dep)));
            }
        }

        build_order_stack
    }

    /// Helper function that finds all idempotent capsules with no nonidempotent downstream
    /// capsules, given a `build_order_stack` (a *reversed build order*).
    ///
    /// While the build order specifies the order in which nodes must be built in to propagate
    /// updates, the reverse of the build order specifies the order in which we can trim down
    /// some fat through gc.
    fn get_disposable_nodes_from_build_order_stack(
        &mut self,
        build_order_stack: &Vec<Id>,
    ) -> HashSet<Id> {
        let mut disposable = HashSet::new();

        for id in build_order_stack {
            let node = self.node_or_panic(id);
            let dependents_all_disposable =
                node.dependents.iter().all(|dep| disposable.contains(dep));
            if node.is_idempotent() && dependents_all_disposable {
                disposable.insert(Arc::clone(id));
            }
        }

        disposable
    }
}
