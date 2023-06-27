use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn capsule(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);

    let (fn_name, capsule_name) = get_fn_and_capsule_names(&input, "Capsule");
    let capsule_type = get_fn_return_type(&input);

    let args = process_capsule_fn_params(&input, |ty| match ty {
        // A dependency capsule
        syn::Type::Path(syn::TypePath { path, .. }) => {
            quote! { #path(reader.read::<#path>()) }
        }
        // The capsule reader (to conditionally watch capsules and read self)
        syn::Type::Reference(_) => quote! { reader },
        // The side effect handle
        syn::Type::ImplTrait(_) => quote! { handle },
        // We don't allow for any other type of parameters
        _ => panic!(concat!(
            "Capsule functions can only consume ",
            "other capsules ( MyCapsule(value): MyCapsule ), ",
            "a capsule reader ( reader: &mut impl CapsuleReader ), and ",
            "a side effect handle ( handle: impl SideEffectHandle )"
        )),
    });

    let capsule_impl = quote! {
        #input

        struct #capsule_name(#capsule_type);

        impl rearch::Capsule for #capsule_name {
            type T = #capsule_type;

            fn build<'a>(
                reader: &mut impl rearch::CapsuleReader<Self::T>,
                handle: impl rearch::SideEffectHandle<'a>
            ) -> Self::T {
                #fn_name(#(#args),*)
            }
        }
    };

    capsule_impl.into()
}

#[proc_macro_attribute]
pub fn factory(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);

    let (fn_name, factory_name) = get_fn_and_capsule_names(&input, "Factory");
    let dependencies = get_dependencies(&input);
    let fn_type = get_fn_return_type(&input);

    let factory_params = {
        let mut iter = process_capsule_fn_params(&input, |ty| match ty {
            syn::Type::Tuple(syn::TypeTuple { elems, .. }) => Some(elems),
            _ => None,
        })
        .flatten();

        let factory_params = iter.next();
        if factory_params.is_some() && iter.next().is_some() {
            panic!("Only up to one set of arguments (represented by a tuple) is permitted!")
        }
        factory_params
    };
    let factory_type = {
        match factory_params {
            Some(ref factory_params) => {
                quote! { std::sync::Arc<dyn Fn(#factory_params) -> #fn_type + Send + Sync> }
            }
            None => quote! { std::sync::Arc<dyn Fn() -> #fn_type + Send + Sync> },
        }
    };
    let factory_args = {
        let factory_args = factory_params.map(|elems| {
            let num_args = elems.iter().count();
            (0..num_args).map(|i| format_ident!("_factory_arg{i}"))
        });
        match factory_args {
            Some(factory_args) => quote! {#(#factory_args),*},
            None => quote! {},
        }
    };

    // We need to store the capsules' state locally to prevent a borrow on the reader
    // (closure will close over the capsules' state but not the reader)
    let local_capsule_vars = (0..dependencies.len()).map(|i| format_ident!("capsule_var{i}"));
    let mut capsule_count = 0;
    let args = process_capsule_fn_params(&input, move |ty| match ty {
        // A dependency capsule
        syn::Type::Path(syn::TypePath { path, .. }) => {
            let local_var_name = format_ident!("capsule_var{capsule_count}");
            capsule_count += 1;
            quote! { #path(#local_var_name) }
        }
        // Factory arguments
        syn::Type::Tuple(syn::TypeTuple { elems, .. }) => {
            let num_args = elems.iter().count();
            let factory_args = (0..num_args).map(|i| format_ident!("_factory_arg{i}"));
            quote! { ( #(#factory_args,)* ) }
        }
        // We don't allow for any other type of parameters
        _ => panic!(concat!(
            "Factory functions can only consume ",
            "parameters to the factory function ( (my_str): (String,) ) ",
            "and other capsules ( MyCapsule(value): MyCapsule )",
        )),
    });

    let capsule_impl = quote! {
        #input

        struct #factory_name(#factory_type);

        impl rearch::Capsule for #factory_name {
            type T = #factory_type;

            fn build<'a>(
                reader: &mut impl rearch::CapsuleReader<Self::T>,
                handle: impl rearch::SideEffectHandle<'a>
            ) -> Self::T {
                #(let #local_capsule_vars = reader.read::<#dependencies>();)*

                std::sync::Arc::new(move |#factory_args| #fn_name(#(#args),*))
            }
        }
    };

    capsule_impl.into()
}

fn get_fn_and_capsule_names(input: &syn::ItemFn, name_suffix: &str) -> (syn::Ident, syn::Ident) {
    let fn_name = input.sig.ident.clone();
    let capsule_name = {
        let mut name = snake_to_pascal(&fn_name.to_string());
        name.push_str(name_suffix);
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

fn get_dependencies(input: &syn::ItemFn) -> Vec<syn::Path> {
    process_capsule_fn_params(input, |ty| match ty {
        syn::Type::Path(syn::TypePath { path, .. }) => Some(path),
        _ => None,
    })
    .flatten()
    .collect()
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
