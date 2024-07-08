// TODO: I would have liked to just have a macro for ServerBound and ClientBound and have
// NetworkMessage be implemented with them, but this is magic to me so I keep it simple.

use proc_macro::TokenStream;

use syn::{
    parse_macro_input,
    DeriveInput,
};
use quote::quote;

#[proc_macro_derive(NetworkMessage)]
pub fn derive_network_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    TokenStream::from(quote! {
        #[typetag::serde]
        impl #impl_generics crate::network_message::NetworkMessage for #struct_name #type_generics #where_clause {
        }
    })
}

#[proc_macro_derive(ServerBound)]
pub fn derive_server_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let struct_name = &ast.ident;
    let message_name = struct_name.to_string();
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics crate::network_message::ServerBound for #struct_name #type_generics #where_clause {
            const NAME: &'static str = #message_name;
        }
    })
}

#[proc_macro_derive(ClientBound)]
pub fn derive_client_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let struct_name = &ast.ident;
    let message_name = struct_name.to_string();
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    TokenStream::from(quote! {
        impl #impl_generics crate::network_message::ClientBound for #struct_name #type_generics #where_clause {
            const NAME: &'static str = #message_name;
        }
    })
}
