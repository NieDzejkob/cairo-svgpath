#![feature(proc_macro_hygiene)]
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

fn get_params(input: TokenStream) -> Option<(proc_macro2::Ident, String)> {
    let input: proc_macro2::TokenStream = input.into();
    let mut input = input.into_iter();
    let ident = input.next()?;
    match input.next()? {
        proc_macro2::TokenTree::Punct(punct) => if punct.as_char() != ',' {
            return None;
        },
        _ => return None,
    }

    let ident = match ident {
        proc_macro2::TokenTree::Ident(ident) => ident,
        _ => return None,
    };

    let path = input.next()?;
    // Handle trailing input
    if let Some(_) = input.next() {
        return None;
    };

    let path = match path {
        proc_macro2::TokenTree::Literal(literal) => syn::Lit::new(literal),
        _ => return None,
    };

    let path = match path {
        syn::Lit::Str(string) => string.value(),
        _ => return None,
    };

    Some((ident, path))
}

#[proc_macro]
pub fn svgpath(input: TokenStream) -> TokenStream {
    let (ident, path) = get_params(input).expect("syntax error");
    let mut path: svgtypes::Path = path.parse().expect("parsing the path failed");
    path.conv_to_absolute();
    use svgtypes::PathSegment::*;
    path.iter().map(|segment| match segment {
        MoveTo { x, y, .. } => quote! { #ident.move_to(#x, #y); },
        CurveTo { x1, y1, x2, y2, x, y, .. } => quote! { #ident.curve_to(#x1, #y1, #x2, #y2, #x, #y); },
        other => {
            dbg!(other);
            unimplemented!();
        }
    }).map(Into::<proc_macro::TokenStream>::into).collect()
}
