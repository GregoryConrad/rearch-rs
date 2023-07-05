use proc_macro::TokenStream;
use quote::{format_ident, quote};
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

    let (fn_name, capsule_name) = get_fn_and_capsule_names(&input);
    let capsule_type = get_fn_return_type(&input);

    let args = process_capsule_fn_params(&input, |ty| match ty {
        // The capsule reader
        syn::Type::Reference(_) => quote! { reader },
        // The side effect handle
        syn::Type::Path(_) => quote! { register },
        // We don't allow for any other type of parameters
        _ => panic!(concat!(
            "Capsule functions can only consume ",
            "a capsule reader ( reader: &mut impl CapsuleReader ) and ",
            "a side effect registrant ( register: SideEffectRegistrar<'_> )"
        )),
    });

    let _is_super_pure =
        process_capsule_fn_params(&input, |ty| !matches!(ty, syn::Type::Path(_))).all(|b| b);

    let capsule_impl = quote! {
        #input

        struct #capsule_name;

        impl rearch::Capsule for #capsule_name {
            type Data = #capsule_type;
            fn build(
                &self,
                reader: &mut impl rearch::CapsuleReader<Data = Self::Data>,
                register: rearch::SideEffectRegistrar<'_>
            ) -> Self::Data {
                #fn_name(#(#args),*)
            }
        }
    };

    capsule_impl.into()
}

fn get_fn_and_capsule_names(input: &syn::ItemFn) -> (syn::Ident, syn::Ident) {
    let fn_name = input.sig.ident.clone();
    let capsule_name = {
        let mut name = snake_to_pascal(&fn_name.to_string());
        name.push_str("Capsule");
        format_ident!("{name}")
    };
    (fn_name, capsule_name)
}

fn get_fn_return_type(input: &syn::ItemFn) -> syn::Type {
    match input.sig.output.clone() {
        syn::ReturnType::Default => panic!("Capsules must return a static or owned type"),
        syn::ReturnType::Type(_, t) => *t,
    }
}

fn process_capsule_fn_params<T, F>(
    func: &syn::ItemFn,
    mut param_mapper: F,
) -> impl Iterator<Item = T>
where
    F: FnMut(syn::Type) -> T,
{
    func.sig
        .inputs
        .clone()
        .into_iter()
        .map(move |param| match param {
            syn::FnArg::Receiver(_) => panic!("Capsule functions must be top-level"),
            syn::FnArg::Typed(syn::PatType { ty, .. }) => param_mapper(*ty),
        })
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut pascal_case = first.to_uppercase().to_string();
                    pascal_case.extend(chars);
                    pascal_case
                }
            }
        })
        .fold(String::new(), |mut total, curr| {
            total.push_str(&curr);
            total
        })
}
