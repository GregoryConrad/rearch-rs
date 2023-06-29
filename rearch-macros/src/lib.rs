use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_macro_input;

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

    let is_super_pure =
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
