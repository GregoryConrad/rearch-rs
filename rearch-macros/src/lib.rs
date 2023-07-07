use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse_macro_input;

/// Macro for handling some implementation boilerplate; do not use.
#[proc_macro]
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
                Box::new(move |mutation: Box<dyn FnOnce(&mut Box<dyn Any + Send>)>| {
                    rebuild_all(Box::new(move |all_states| {
                        mutation(all_states.#i.get_mut().expect(EFFECT_FAILED_CAST_MSG));
                    }));
                })
            }))
        }
    });
    let effect_impl = quote! {
        impl<'a, #(#types: SideEffect<'a>),*> SideEffect<'a> for (#(#types),*) {
            type Api = (#(#types::Api),*);

            #[allow(clippy::unused_unit)]
            fn build(self, registrar: SideEffectRegistrar<'a>) -> Self::Api {
                let (all_states, rebuild_all) = registrar.raw((
                    #(#once_cell_inits),*
                ));
                (#(#individual_apis),*)
            }
        }
    };
    effect_impl.into()
}

#[proc_macro_attribute]
pub fn capsule(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);

    let fn_name = input.sig.ident.clone();
    let fn_visibility = input.vis.clone();
    let fn_ret_type = get_fn_return_type(&input);
    let fn_registrar_parameter = get_fn_registrar_parameter(&input)
        .map(|param| quote! { #param })
        .unwrap_or(quote! { _: rearch::SideEffectRegistrar });
    let fn_body: proc_macro2::TokenStream = {
        let original = input.block.to_token_stream().to_string();
        let re = regex::Regex::new(r"[^\w]_(\w+)").unwrap();
        re.replace_all(&original, "__reader.read($1)")
            .parse()
            .unwrap()
    };

    let capsule_impl = quote! {
        #fn_visibility fn #fn_name(
            mut __reader: rearch::CapsuleReader,
            #fn_registrar_parameter,
        ) -> #fn_ret_type {
            #fn_body
        }
    };

    capsule_impl.into()
}

fn get_fn_return_type(input: &syn::ItemFn) -> syn::Type {
    match input.sig.output.clone() {
        syn::ReturnType::Default => panic!("Capsules must return a static or owned type"),
        syn::ReturnType::Type(_, t) => *t,
    }
}

fn get_fn_registrar_parameter(func: &syn::ItemFn) -> Option<syn::PatType> {
    let mut paths = func
        .sig
        .inputs
        .clone()
        .into_iter()
        .map(move |param| match param {
            syn::FnArg::Receiver(_) => panic!("Macro capsule functions must be top-level"),
            syn::FnArg::Typed(pat) => pat,
        });
    let registrar_name = paths.next();
    if registrar_name.is_some() && paths.next().is_some() {
        panic!("Macro capsule functions may only consume 1 parameter, the SideEffectRegistrar");
    }
    registrar_name
}
