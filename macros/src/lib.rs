use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, ItemStruct, LitInt, Token, Type};
use syn::parse::{Parse, ParseStream, Result};



#[proc_macro_attribute]
pub fn vertex(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemStruct);
    let name = &input.ident;

    // Only allow named-field structs
    let fields = match &input.fields {
        syn::Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => panic!("#[vertex] can only be used on structs with named fields"),
    };

    // Compute field offsets
    let mut offset_exprs = Vec::new();
    let mut current_offset = quote! { 0usize };

    for (i, field) in fields.iter().enumerate() {
        offset_exprs.push(current_offset.clone());
        if i < fields.len() - 1 {
            let ty = &field.ty;
            current_offset = quote! {
                #current_offset + ::std::mem::size_of::<#ty>()
            };
        }
    }

    // Build VertexAttribute array
    let attributes = fields.iter().zip(offset_exprs.clone()).enumerate().map(|(i, (f, offset))| {
        let ty = &f.ty;
        let location = i as u32;
        quote! {
            wgpu::VertexAttribute {
                offset: (#offset) as u64,
                shader_location: #location,
                format: <#ty as wgpui::AsVertexFormat>::VERTEX_FORMAT,
            }
        }
    });

    let member_names = fields.iter().map(|f| {
        f.ident.as_ref().unwrap().to_string()
    });

    // CamelCase -> snake_case
    let label = name.to_string()
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i != 0 {
                format!("_{}", c.to_ascii_lowercase())
            } else {
                c.to_ascii_lowercase().to_string()
            }
        })
        .collect::<String>();

    let expanded = quote! {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        #input

        impl wgpui::Vertex for #name {
            const VERTEX_LABEL: &'static str = #label;
            const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute] = &[
                #(#attributes, )*
            ];
            const VERTEX_MEMBERS: &'static [&'static str] = &[
                #(#member_names, )*
            ];
        }
    };

    TokenStream::from(expanded)
}


#[proc_macro_attribute]
pub fn wgsl(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse attribute argument as identifier
    let wgsl_ident = parse_macro_input!(attr as Ident);
    let wgsl_name_str = wgsl_ident.to_string();

    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let fields = match &input.fields {
        syn::Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => panic!("#[wgsl] can only be used on structs with named fields"),
    };

    // compute offsets
    let mut offset_exprs = Vec::new();
    let mut current_offset = quote! { 0usize };

    for (i, field) in fields.iter().enumerate() {
        offset_exprs.push(current_offset.clone());
        if i < fields.len() - 1 {
            let ty = &field.ty;
            current_offset = quote! {
                #current_offset + ::std::mem::size_of::<#ty>()
            };
        }
    }

    // build VertexAttribute array
    let attributes = fields.iter().zip(offset_exprs.clone()).enumerate().map(|(i, (f, offset))| {
        let ty = &f.ty;
        let location = i as u32;
        quote! {
            wgpu::VertexAttribute {
                offset: (#offset) as u64,
                shader_location: #location,
                format: <#ty as wgpui::AsVertexFormat>::VERTEX_FORMAT,
            }
        }
    });

    let member_names = fields.iter().map(|f| {
        f.ident.as_ref().unwrap().to_string()
    });

    // CamelCase -> snake_case label
    let label = name.to_string()
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i != 0 {
                format!("_{}", c.to_ascii_lowercase())
            } else {
                c.to_ascii_lowercase().to_string()
            }
        })
        .collect::<String>();

    let expanded = quote! {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        #input

        impl wgpui::Vertex for #name {
            const VERTEX_LABEL: &'static str = #label;
            const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute] = &[
                #(#attributes, )*
            ];
            const VERTEX_MEMBERS: &'static [&'static str] = &[
                #(#member_names, )*
            ];
            const WGSL_NAME: &'static str = #wgsl_name_str;
        }
    };

    TokenStream::from(expanded)
}
