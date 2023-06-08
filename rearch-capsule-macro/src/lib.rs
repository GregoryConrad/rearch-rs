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
        // TODO allow an Option<&Self::T> for reader.read_self()!!
        syn::Type::Path(syn::TypePath { path, .. }) => {
            quote! { #path(reader.read::<#path>().as_ref()) }
        }
        // The side effect handle
        syn::Type::Reference(_) => quote! { handle },
        // We don't allow for any other type of parameters
        _ => panic!(concat!(
            "Capsule functions can only consume ",
            "other capsules (MyCapsule(value): MyCapsule) and ",
            "a side effect handle (use: &mut impl SideEffectHandle)"
        )),
    });

    let capsule_impl = quote! {
        #input

        struct #capsule_name<'a>(&'a #capsule_type);

        impl<'a> rearch::Capsule for #capsule_name<'a> {
            type T = #capsule_type;

            fn build(reader: &mut impl rearch::CapsuleReader<Self::T>,
                handle: &mut impl rearch::SideEffectHandle) -> Self::T {
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
                quote! { Box<dyn Fn(#factory_params) -> #fn_type + Sync + Send> }
            }
            None => quote! { Box<dyn Fn() -> #fn_type + Sync + Send> },
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

    // We need to store an Arc of the capsules locally to prevent a borrow on the reader
    // (closure will close over the Arcs but not the reader)
    let local_capsule_vars = (0..dependencies.len()).map(|i| format_ident!("capsule_var{i}"));
    let mut capsule_count = 0;
    let args = process_capsule_fn_params(&input, move |ty| match ty {
        // A dependency capsule
        // TODO Option<&Self::T> for reader.read_self()!!
        syn::Type::Path(syn::TypePath { path, .. }) => {
            let local_var_name = format_ident!("capsule_var{capsule_count}");
            capsule_count += 1;
            quote! { #path(#local_var_name.as_ref()) }
        }
        // The side effect handle
        syn::Type::Reference(_) => quote! { handle },
        // Factory arguments
        syn::Type::Tuple(syn::TypeTuple { elems, .. }) => {
            let num_args = elems.iter().count();
            let factory_args = (0..num_args).map(|i| format_ident!("_factory_arg{i}"));
            quote! { ( #(#factory_args,)* ) }
        }
        // We don't allow for any other type of parameters
        _ => panic!(concat!(
            "Factory functions can only consume ",
            "parameters to the factory function ((my_str): (String,)), ",
            "other capsules (MyCapsule(value): MyCapsule), and ",
            "a side effect handle (use: &mut impl SideEffectHandle)"
        )),
    });

    let capsule_impl = quote! {
        #input

        struct #factory_name<'a>(&'a #factory_type);

        impl<'a> rearch::Capsule for #factory_name<'a> {
            type T = #factory_type;

            fn build(reader: &mut impl rearch::CapsuleReader<Self::T>,
                handle: &mut impl rearch::SideEffectHandle) -> Self::T {
                #(let #local_capsule_vars = reader.read::<#dependencies>();)*

                Box::new(move |#factory_args| #fn_name(#(#args),*))
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

trait StringJoin: Iterator<Item = String> {
    fn join(self) -> String
    where
        Self: Sized,
    {
        self.fold(String::new(), |mut total, curr| {
            total.push_str(&curr);
            total
        })
    }
}
impl<I: Iterator<Item = String>> StringJoin for I {}

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
        .join()
}
