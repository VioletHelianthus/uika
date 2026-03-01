// #[uclass_impl] macro: generates function registration for #[ufunction] methods
// on a Rust-defined UE class.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Meta, ReturnType, Token, Type};
use syn::punctuated::Punctuated;

use crate::prop_type;
use crate::uclass::{to_snake_case, to_screaming_snake};

// ---------------------------------------------------------------------------
// Parsed ufunction info
// ---------------------------------------------------------------------------

struct ParamInfo {
    rust_name: Ident,
    ue_name: String,
    rust_ty: Type,
}

struct UFunctionInfo {
    method_ident: Ident,
    ue_name: String,
    params: Vec<ParamInfo>,
    return_type: Option<ParamInfo>,
    is_mut: bool,
    is_override: bool,
}

// ---------------------------------------------------------------------------
// Main expansion
// ---------------------------------------------------------------------------

pub fn expand_uclass_impl(_attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let input: ItemImpl = parse2(item)?;

    // Extract struct name from impl target
    let struct_name = match &*input.self_ty {
        Type::Path(tp) => tp.path.segments.last()
            .ok_or_else(|| syn::Error::new_spanned(&input.self_ty, "expected type name"))?
            .ident.clone(),
        _ => return Err(syn::Error::new_spanned(&input.self_ty, "expected type name")),
    };

    let struct_name_str = struct_name.to_string();
    let rust_data_name = format_ident!("__{}RustData", struct_name);
    let class_handle_name = format_ident!("__UIKA_CLASS_HANDLE_{}", to_screaming_snake(&struct_name_str));

    // Classify methods: collect #[ufunction] info, strip attrs
    let mut ufunctions: Vec<UFunctionInfo> = Vec::new();
    let mut clean_impl = input.clone();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            let has_ufunction = method.attrs.iter().any(|a| a.path().is_ident("ufunction"));
            if has_ufunction {
                ufunctions.push(parse_ufunction(method)?);
            }
        }
    }

    // Strip #[ufunction] attrs from the emitted impl block
    for item in &mut clean_impl.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|a| !a.path().is_ident("ufunction"));
        }
    }

    // Generate register_functions body
    let register_fns_name = format_ident!(
        "__uika_register_{}_functions",
        to_snake_case(&struct_name_str),
    );

    let mut register_stmts: Vec<TokenStream> = Vec::new();

    for uf in &ufunctions {
        let ue_name = &uf.ue_name;
        let ue_name_bytes = ue_name.as_bytes();
        let ue_name_len = ue_name.len() as u32;
        let method_ident = &uf.method_ident;

        let flags_expr = if uf.is_override {
            quote! {
                ::uika::ffi::FUNC_NATIVE | ::uika::ffi::FUNC_BLUEPRINT_EVENT | ::uika::ffi::FUNC_PUBLIC
            }
        } else {
            quote! {
                ::uika::ffi::FUNC_NATIVE | ::uika::ffi::FUNC_BLUEPRINT_CALLABLE | ::uika::ffi::FUNC_PUBLIC
            }
        };

        // Total number of offsets to cache (params + optional return)
        let total_offsets = uf.params.len() + if uf.return_type.is_some() { 1 } else { 0 };

        // Generate offset init expressions
        let mut offset_inits: Vec<TokenStream> = Vec::new();
        for param in &uf.params {
            let param_ue_name = &param.ue_name;
            let param_ue_bytes = param_ue_name.as_bytes();
            let param_ue_len = param_ue_name.len() as u32;
            offset_inits.push(quote! {
                {
                    let p = ::uika::runtime::ffi_dispatch::reflection_get_function_param(
                        func,
                        [#(#param_ue_bytes),*].as_ptr(),
                        #param_ue_len,
                    );
                    ::uika::runtime::ffi_dispatch::reflection_get_property_offset(p)
                }
            });
        }
        if uf.return_type.is_some() {
            offset_inits.push(quote! {
                {
                    let p = ::uika::runtime::ffi_dispatch::reflection_get_function_param(
                        func,
                        b"ReturnValue".as_ptr(),
                        11u32,
                    );
                    ::uika::runtime::ffi_dispatch::reflection_get_property_offset(p)
                }
            });
        }

        // Generate param reads from the params buffer (via native_mem_read for wasm32 compat)
        let mut param_reads: Vec<TokenStream> = Vec::new();
        let mut param_idents: Vec<&Ident> = Vec::new();
        for (i, param) in uf.params.iter().enumerate() {
            let rust_name = &param.rust_name;
            let rust_ty = &param.rust_ty;
            let idx = syn::Index::from(i);
            param_reads.push(quote! {
                let #rust_name: #rust_ty = unsafe {
                    ::uika::runtime::ffi_dispatch::native_mem_read::<#rust_ty>(params, offsets[#idx] as usize)
                };
            });
            param_idents.push(rust_name);
        }

        // Generate return value zero-init (before user call) and write (after).
        // Zero-init ensures C++ reads 0/false instead of garbage if the user
        // method panics and ffi_boundary catches the unwind.
        let (return_zero_init, return_write) = if let Some(ref ret) = uf.return_type {
            let ret_ty = &ret.rust_ty;
            let ret_idx = syn::Index::from(uf.params.len());
            (
                quote! {
                    unsafe {
                        ::uika::runtime::ffi_dispatch::native_mem_write(
                            params, offsets[#ret_idx] as usize,
                            unsafe { std::mem::zeroed::<#ret_ty>() },
                        );
                    }
                },
                quote! {
                    unsafe {
                        ::uika::runtime::ffi_dispatch::native_mem_write(params, offsets[#ret_idx] as usize, __ret);
                    }
                },
            )
        } else {
            (quote! {}, quote! {})
        };

        // Construct this binding
        let this_binding = if uf.is_mut {
            quote! {
                let mut __this = #struct_name {
                    __obj: obj,
                    __rust_data: rust_data as *mut #rust_data_name,
                };
            }
        } else {
            quote! {
                let __this = #struct_name {
                    __obj: obj,
                    __rust_data: rust_data as *mut #rust_data_name,
                };
            }
        };

        // Method call expression
        let call_expr = if uf.return_type.is_some() {
            quote! { let __ret = __this.#method_ident(#(#param_idents),*); }
        } else {
            quote! { __this.#method_ident(#(#param_idents),*); }
        };

        // Register callback and add function + params
        let func_var = format_ident!("__func_{}", method_ident);

        register_stmts.push(quote! {
            let __callback_id = {
                let callback_id = ::uika::runtime::reify_registry::register_function(
                    move |obj: ::uika::ffi::UObjectHandle, rust_data: *mut u8, params: ::uika::runtime::ffi_dispatch::NativePtr| {
                        static OFFSETS: std::sync::OnceLock<[u32; #total_offsets]> = std::sync::OnceLock::new();
                        let offsets = OFFSETS.get_or_init(|| unsafe {
                            let cls = <#struct_name as ::uika::runtime::UeClass>::static_class();
                            let func = ::uika::runtime::ffi_dispatch::reflection_find_function_by_class(
                                cls,
                                [#(#ue_name_bytes),*].as_ptr(),
                                #ue_name_len,
                            );
                            [#(#offset_inits),*]
                        });
                        #(#param_reads)*
                        #return_zero_init
                        #this_binding
                        #call_expr
                        #return_write
                    }
                );
                callback_id
            };

            let #func_var = unsafe {
                ::uika::runtime::ffi_dispatch::reify_add_function(
                    cls,
                    [#(#ue_name_bytes),*].as_ptr(),
                    #ue_name_len,
                    __callback_id,
                    #flags_expr,
                )
            };
        });

        // Add function params
        for param in &uf.params {
            let info = prop_type::map_type(&param.rust_ty).unwrap();
            let param_ue_name = &param.ue_name;
            let param_ue_bytes = param_ue_name.as_bytes();
            let param_ue_len = param_ue_name.len() as u32;
            let prop_type_expr = &info.prop_type_expr;

            register_stmts.push(quote! {
                unsafe {
                    ::uika::runtime::ffi_dispatch::reify_add_function_param(
                        #func_var,
                        [#(#param_ue_bytes),*].as_ptr(),
                        #param_ue_len,
                        #prop_type_expr as u32,
                        ::uika::ffi::CPF_PARM,
                        std::ptr::null(),
                    );
                }
            });
        }

        // Add return param if any
        if let Some(ref ret) = uf.return_type {
            let info = prop_type::map_type(&ret.rust_ty).unwrap();
            let prop_type_expr = &info.prop_type_expr;

            register_stmts.push(quote! {
                unsafe {
                    ::uika::runtime::ffi_dispatch::reify_add_function_param(
                        #func_var,
                        b"ReturnValue".as_ptr(),
                        11u32,
                        #prop_type_expr as u32,
                        ::uika::ffi::CPF_PARM | ::uika::ffi::CPF_OUT_PARM | ::uika::ffi::CPF_RETURN_PARM,
                        std::ptr::null(),
                    );
                }
            });
        }
    }

    let register_functions_fn = quote! {
        #[doc(hidden)]
        pub fn #register_fns_name() {
            let cls = match #class_handle_name.get() {
                Some(&c) if !c.is_null() => c,
                _ => return,
            };
            #(#register_stmts)*
        }
    };

    Ok(quote! {
        #clean_impl
        #register_functions_fn

        ::uika::__inventory::submit! {
            ::uika::runtime::reify_registry::ClassFunctionRegistration {
                register_functions: #register_fns_name,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_ufunction(method: &ImplItemFn) -> syn::Result<UFunctionInfo> {
    let specifiers = if let Some(attr) = method.attrs.iter().find(|a| a.path().is_ident("ufunction")) {
        parse_ufunction_specifiers(attr)?
    } else {
        Vec::new()
    };
    let is_override = specifiers.iter().any(|s| s == "Override");

    let method_ident = method.sig.ident.clone();
    let ue_name = prop_type::to_pascal_case(&method_ident.to_string());

    // Check for self receiver and its mutability
    let is_mut = method.sig.inputs.first().map_or(false, |arg| {
        matches!(arg, FnArg::Receiver(r) if r.mutability.is_some())
    });

    // Parse params (skip self)
    let mut params = Vec::new();
    for arg in &method.sig.inputs {
        match arg {
            FnArg::Receiver(_) => continue,
            FnArg::Typed(pat_type) => {
                let name = match &*pat_type.pat {
                    syn::Pat::Ident(pi) => pi.ident.clone(),
                    _ => return Err(syn::Error::new_spanned(
                        &pat_type.pat,
                        "ufunction params must be simple identifiers",
                    )),
                };
                let ty = (*pat_type.ty).clone();
                if prop_type::map_type(&ty).is_none() {
                    return Err(syn::Error::new_spanned(
                        &ty,
                        "unsupported ufunction parameter type: only bool/i32/i64/u8/f32/f64 are supported",
                    ));
                }
                let ue_name = prop_type::to_pascal_case(&name.to_string());
                params.push(ParamInfo { rust_name: name, ue_name, rust_ty: ty });
            }
        }
    }

    // Parse return type
    let return_type = match &method.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            if let Type::Tuple(tuple) = &**ty {
                if tuple.elems.is_empty() {
                    None
                } else {
                    return Err(syn::Error::new_spanned(ty, "tuple return types not supported"));
                }
            } else {
                if prop_type::map_type(ty).is_none() {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "unsupported ufunction return type: only bool/i32/i64/u8/f32/f64 are supported",
                    ));
                }
                Some(ParamInfo {
                    rust_name: Ident::new("ReturnValue", proc_macro2::Span::call_site()),
                    ue_name: "ReturnValue".to_string(),
                    rust_ty: (**ty).clone(),
                })
            }
        }
    };

    Ok(UFunctionInfo {
        method_ident,
        ue_name,
        params,
        return_type,
        is_mut,
        is_override,
    })
}

fn parse_ufunction_specifiers(attr: &syn::Attribute) -> syn::Result<Vec<String>> {
    let mut specifiers = Vec::new();
    if let Ok(nested) = attr.parse_args_with(
        Punctuated::<Meta, Token![,]>::parse_terminated,
    ) {
        for meta in &nested {
            if let Meta::Path(p) = meta {
                if let Some(ident) = p.get_ident() {
                    specifiers.push(ident.to_string());
                }
            }
        }
    }
    Ok(specifiers)
}
