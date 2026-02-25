// Rust type → UikaReifyPropType mapping + PropertyApi accessor names.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

/// Info about a mapped UE property type.
pub struct PropTypeInfo {
    /// Token for the UikaReifyPropType variant (e.g. `UikaReifyPropType::Float`).
    pub prop_type_expr: TokenStream,
    /// The Rust type used for the getter out-variable and setter parameter.
    pub rust_type: TokenStream,
    /// Identifier for the PropertyApi getter (e.g. `get_f32`).
    pub getter_fn: syn::Ident,
    /// Identifier for the PropertyApi setter (e.g. `set_f32`).
    pub setter_fn: syn::Ident,
    /// Default zero-value expression for the getter's out variable.
    pub zero_expr: TokenStream,
}

/// Try to map a Rust type to UE property type info.
/// Returns None for unsupported types.
pub fn map_type(ty: &Type) -> Option<PropTypeInfo> {
    let type_str = match ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            seg.ident.to_string()
        }
        _ => return None,
    };

    let ident = |s: &str| syn::Ident::new(s, proc_macro2::Span::call_site());

    let info = match type_str.as_str() {
        "bool" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::Bool },
            rust_type: quote! { bool },
            getter_fn: ident("get_bool"),
            setter_fn: ident("set_bool"),
            zero_expr: quote! { false },
        },
        "i32" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::Int32 },
            rust_type: quote! { i32 },
            getter_fn: ident("get_i32"),
            setter_fn: ident("set_i32"),
            zero_expr: quote! { 0i32 },
        },
        "i64" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::Int64 },
            rust_type: quote! { i64 },
            getter_fn: ident("get_i64"),
            setter_fn: ident("set_i64"),
            zero_expr: quote! { 0i64 },
        },
        "u8" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::UInt8 },
            rust_type: quote! { u8 },
            getter_fn: ident("get_u8"),
            setter_fn: ident("set_u8"),
            zero_expr: quote! { 0u8 },
        },
        "f32" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::Float },
            rust_type: quote! { f32 },
            getter_fn: ident("get_f32"),
            setter_fn: ident("set_f32"),
            zero_expr: quote! { 0.0f32 },
        },
        "f64" => PropTypeInfo {
            prop_type_expr: quote! { ::uika::ffi::UikaReifyPropType::Double },
            rust_type: quote! { f64 },
            getter_fn: ident("get_f64"),
            setter_fn: ident("set_f64"),
            zero_expr: quote! { 0.0f64 },
        },
        _ => return None,
    };
    Some(info)
}

/// Convert snake_case field name to PascalCase UE property name.
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Compile-time FNV-1a hash of a byte string, producing u64.
pub fn fnv1a_hash(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &b in s.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
