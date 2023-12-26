use proc_macro::TokenStream;
use quote::{format_ident, quote};

/// Macro for handling some implementation boilerplate; do not use.
#[proc_macro]
#[allow(clippy::missing_panics_doc)]
pub fn generate_tuple_side_effect_impl(input: TokenStream) -> TokenStream {
    let types = input
        .into_iter()
        .map(|token| match token {
            proc_macro::TokenTree::Ident(ident) => ident,
            _ => panic!("Expected identifier as argument"),
        })
        .map(|token| token.to_string())
        .map(|ident| format_ident!("{ident}"))
        .collect::<Vec<_>>();
    let once_cell_inits = (0..types.len()).map(|_| quote! { OnceCell::new() });
    let individual_apis = (0..types.len()).map(syn::Index::from).map(|i| {
        quote! {
            self.#i.build(SideEffectRegistrar::new(&mut all_states.#i, {
                let rebuild_all = rebuild_all.clone();
                Arc::new(move |mutation: Box<dyn FnOnce(&mut dyn Any)>| {
                    rebuild_all(Box::new(move |all_states| {
                        mutation(all_states.#i.get_mut().expect(EFFECT_FAILED_CAST_MSG).as_mut());
                    }));
                })
            }, Arc::clone(&run_txn)))
        }
    });
    let effect_impl = quote! {
        impl<'a, #(#types: SideEffect<'a>),*> SideEffect<'a> for (#(#types),*) {
            type Api = (#(#types::Api),*);

            #[allow(clippy::unused_unit)]
            fn build(self, registrar: SideEffectRegistrar<'a>) -> Self::Api {
                let (all_states, rebuild_all, run_txn) = registrar.raw((
                    #(#once_cell_inits),*
                ));
                (#(#individual_apis),*)
            }
        }
    };
    effect_impl.into()
}
