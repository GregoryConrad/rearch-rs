use crate::ContainerWriteTxn;
impl ContainerWriteTxn<'_> {
    // TODO maybe we can expose a singular method to do this in just one method in a new file
    /*
    #[must_use]
    .start_garbage_collection()
        // following few
        .try_single_only(capsule)
        .try_single_and_dependents(capsule)
        .force_single_only(capsule) // unsafe
        .force_single_and_dependents(capsule) // unsafe
        // are the following two even possible easily?
        .trim_dependencies()
        .dont_trim_dependencies()
        // and then if we want to thourough
        .all_super_pure()
        // better name for this one:
        .commit(); // returns enum of not present, validation failed, success
    // validation failed could have status where self as ip/sp and dependents as dne/ip/sp
    */
    /*
    /// Attempts to garbage collect the given Capsule and its dependent subgraph, disposing
    /// the supplied Capsule and its dependent subgraph (and then returning `true`) only when
    /// the supplied Capsule and its dependent subgraph consist only of super pure capsules.
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
