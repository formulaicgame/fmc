use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ServerBound)]
pub fn derive_server_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics crate::network_message::ServerBound for #struct_name #type_generics #where_clause {
            const TYPE: crate::MessageType = crate::MessageType::#struct_name;
        }
    })
}

#[proc_macro_derive(ClientBound)]
pub fn derive_client_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics crate::network_message::ClientBound for #struct_name #type_generics #where_clause {
            const TYPE: crate::MessageType = crate::MessageType::#struct_name;
        }
    })
}
