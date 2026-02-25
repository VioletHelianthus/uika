// Core uclass macro expansion: parses #[uclass(parent = Type)] struct with
// #[uproperty(...)] fields and generates the full reification boilerplate.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, Expr, Fields, Ident, ItemStruct, Meta, Token};
use syn::punctuated::Punctuated;

use crate::prop_type;

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

/// Parsed #[uclass(...)] attributes.
struct UClassArgs {
    parent_path: syn::Path,  // Full Rust path for compile-time type checking
    parent_name: String,     // Last segment string for runtime find_class
}

fn parse_uclass_args(attr: TokenStream) -> syn::Result<UClassArgs> {
    let metas: Punctuated<Meta, Token![,]> =
        parse2::<syn::parse::Nothing>(attr.clone())
            .map(|_| Punctuated::new())
            .unwrap_or_else(|_| {
                syn::parse::Parser::parse2(
                    Punctuated::<Meta, Token![,]>::parse_terminated,
                    attr,
                )
                .unwrap_or_default()
            });

    let mut parent_path: Option<syn::Path> = None;
    for meta in &metas {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("parent") {
                if let Expr::Path(expr_path) = &nv.value {
                    parent_path = Some(expr_path.path.clone());
                } else {
                    return Err(syn::Error::new_spanned(
                        &nv.value,
                        "`parent` must be a type path, not a string literal.\n\n\
                         Example: #[uclass(parent = Actor)]",
                    ));
                }
            }
        }
    }
    let parent_path = parent_path.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[uclass] requires a `parent` attribute specifying the UE parent class.\n\n\
             Example:\n\
             \x20   #[uclass(parent = Actor)]\n\
             \x20   pub struct MyActor { ... }\n\n\
             Common parents: Actor, Pawn, Character, PlayerController,\n\
             \x20               GameModeBase, ActorComponent, SceneComponent",
        )
    })?;
    let parent_name = parent_path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    Ok(UClassArgs { parent_path, parent_name })
}

/// Specifiers parsed from #[uproperty(...)].
///
/// **CDO default note**: The `default = ...` value is written to the CDO during
/// class registration. Hot-reloading will NOT propagate new defaults to existing
/// instances. Always initialize critical values in your `receive_begin_play` or
/// init function instead of relying solely on CDO defaults.
#[derive(Default)]
struct UPropertyArgs {
    blueprint_read_write: bool,
    blueprint_read_only: bool,
    edit_anywhere: bool,
    default_expr: Option<Expr>,
}

fn parse_uproperty_args(attr: &syn::Attribute) -> syn::Result<UPropertyArgs> {
    let mut args = UPropertyArgs::default();
    let nested = attr.parse_args_with(
        Punctuated::<Meta, Token![,]>::parse_terminated,
    )?;
    for meta in &nested {
        match meta {
            Meta::Path(p) => {
                if p.is_ident("BlueprintReadWrite") {
                    args.blueprint_read_write = true;
                } else if p.is_ident("BlueprintReadOnly") {
                    args.blueprint_read_only = true;
                } else if p.is_ident("EditAnywhere") {
                    args.edit_anywhere = true;
                }
            }
            Meta::NameValue(nv) => {
                if nv.path.is_ident("default") {
                    args.default_expr = Some(nv.value.clone());
                }
            }
            _ => {}
        }
    }
    Ok(args)
}

// ---------------------------------------------------------------------------
// Component attribute parsing
// ---------------------------------------------------------------------------

/// Parsed #[component(...)] attributes.
struct ComponentArgs {
    is_root: bool,
    attach_to: Option<String>,
}

fn parse_component_args(attr: &syn::Attribute) -> syn::Result<ComponentArgs> {
    let mut args = ComponentArgs {
        is_root: false,
        attach_to: None,
    };

    // #[component] with no parens → defaults
    let nested = match attr.parse_args_with(
        Punctuated::<Meta, Token![,]>::parse_terminated,
    ) {
        Ok(n) => n,
        Err(_) => return Ok(args),
    };

    for meta in &nested {
        match meta {
            Meta::Path(p) => {
                if p.is_ident("root") {
                    args.is_root = true;
                }
            }
            Meta::NameValue(nv) => {
                if nv.path.is_ident("attach") {
                    if let Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s), ..
                    }) = &nv.value
                    {
                        args.attach_to = Some(s.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            &nv.value,
                            "attach must be a string literal, e.g. attach = \"root_scene\"",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(args)
}

// ---------------------------------------------------------------------------
// Field classification
// ---------------------------------------------------------------------------

struct UPropertyField {
    ident: Ident,
    ty: syn::Type,
    args: UPropertyArgs,
}

struct RustPrivateField {
    ident: Ident,
    ty: syn::Type,
}

struct ComponentField {
    ident: Ident,
    component_type: syn::Path,
    is_root: bool,
    attach_to: Option<String>,
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

pub fn expand_uclass(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let args = parse_uclass_args(attr)?;
    let input: ItemStruct = parse2(item)?;

    let struct_name = &input.ident;
    let struct_vis = &input.vis;
    let struct_name_str = struct_name.to_string();

    // Classify fields
    let mut uprops: Vec<UPropertyField> = Vec::new();
    let mut rust_fields: Vec<RustPrivateField> = Vec::new();
    let mut components: Vec<ComponentField> = Vec::new();

    let fields = match &input.fields {
        Fields::Named(f) => &f.named,
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "#[uclass] requires a struct with named fields.\n\n\
                 Example:\n\
                 \x20   #[uclass(parent = Actor)]\n\
                 \x20   pub struct MyActor {\n\
                 \x20       #[uproperty(BlueprintReadWrite, EditAnywhere)]\n\
                 \x20       health: f32,\n\
                 \x20   }",
            ));
        }
    };

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap().clone();
        let field_ty = field.ty.clone();

        let comp_attr = field
            .attrs
            .iter()
            .find(|a| a.path().is_ident("component"));

        let uprop_attr = field
            .attrs
            .iter()
            .find(|a| a.path().is_ident("uproperty"));

        if let Some(attr) = comp_attr {
            let cargs = parse_component_args(attr)?;
            // Extract the type as a path for UeClass trait bound
            let component_type = match &field_ty {
                syn::Type::Path(tp) => tp.path.clone(),
                _ => {
                    return Err(syn::Error::new_spanned(
                        &field_ty,
                        "#[component] field type must be a path (e.g. SceneComponent)",
                    ));
                }
            };
            components.push(ComponentField {
                ident: field_ident,
                component_type,
                is_root: cargs.is_root,
                attach_to: cargs.attach_to,
            });
        } else if let Some(attr) = uprop_attr {
            let pargs = parse_uproperty_args(attr)?;
            // Validate type is supported
            if prop_type::map_type(&field_ty).is_none() {
                return Err(syn::Error::new_spanned(
                    &field_ty,
                    format!("unsupported uproperty type: only bool/i32/i64/u8/f32/f64 are supported in 9b"),
                ));
            }
            uprops.push(UPropertyField {
                ident: field_ident,
                ty: field_ty,
                args: pargs,
            });
        } else {
            rust_fields.push(RustPrivateField {
                ident: field_ident,
                ty: field_ty,
            });
        }
    }

    // Generate names
    let rust_data_name = format_ident!("__{}RustData", struct_name);
    let class_handle_name = format_ident!("__UIKA_CLASS_HANDLE_{}", to_screaming_snake(&struct_name_str));
    let register_fn_name = format_ident!("__uika_register_{}", to_snake_case(&struct_name_str));
    let finalize_fn_name = format_ident!("__uika_finalize_{}", to_snake_case(&struct_name_str));

    let type_id_value = prop_type::fnv1a_hash(&struct_name_str);

    // --- 1. Rewritten user struct (thin handle) ---
    let user_struct = quote! {
        #struct_vis struct #struct_name {
            #[doc(hidden)]
            pub __obj: ::uika::ffi::UObjectHandle,
            #[doc(hidden)]
            pub __rust_data: *mut #rust_data_name,
        }
    };

    // --- 2. Rust private data struct ---
    let rust_data_fields: Vec<TokenStream> = rust_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            quote! { pub #ident: #ty, }
        })
        .collect();

    let rust_data_defaults: Vec<TokenStream> = rust_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { #ident: Default::default(), }
        })
        .collect();

    let rust_data_struct = quote! {
        #[doc(hidden)]
        pub struct #rust_data_name {
            #(#rust_data_fields)*
        }

        impl Default for #rust_data_name {
            fn default() -> Self {
                Self {
                    #(#rust_data_defaults)*
                }
            }
        }
    };

    // --- 3. Static class handle ---
    let static_handle = quote! {
        #[doc(hidden)]
        pub static #class_handle_name: std::sync::OnceLock<::uika::ffi::UClassHandle> = std::sync::OnceLock::new();
    };

    // --- 4. UeClass trait impl ---
    let ue_class_impl = quote! {
        impl ::uika::runtime::UeClass for #struct_name {
            fn static_class() -> ::uika::ffi::UClassHandle {
                *#class_handle_name.get().expect(concat!(stringify!(#struct_name), " not yet registered"))
            }
        }
    };

    // --- 5. Property getters/setters ---
    let mut accessor_methods: Vec<TokenStream> = Vec::new();

    for prop in &uprops {
        let info = prop_type::map_type(&prop.ty).unwrap();
        let field_ident = &prop.ident;
        let ue_name = prop_type::to_pascal_case(&field_ident.to_string());
        let ue_name_bytes = ue_name.as_bytes();
        let ue_name_len = ue_name.len() as u32;
        let rust_ty = &info.rust_type;
        let zero = &info.zero_expr;
        let getter_fn = &info.getter_fn;
        let setter_fn = &info.setter_fn;

        // Getter (always generated)
        let getter_ident = format_ident!("{}", field_ident);
        accessor_methods.push(quote! {
            pub fn #getter_ident(&self) -> #rust_ty {
                static PROP: std::sync::OnceLock<::uika::ffi::FPropertyHandle> = std::sync::OnceLock::new();
                let prop = *PROP.get_or_init(|| unsafe {
                    ((*::uika::runtime::api().reflection).find_property)(
                        <Self as ::uika::runtime::UeClass>::static_class(),
                        [#(#ue_name_bytes),*].as_ptr(),
                        #ue_name_len,
                    )
                });
                let mut val: #rust_ty = #zero;
                unsafe { ((*::uika::runtime::api().property).#getter_fn)(self.__obj, prop, &mut val); }
                val
            }
        });

        // Setter (only if not read-only)
        if !prop.args.blueprint_read_only {
            let setter_ident = format_ident!("set_{}", field_ident);
            accessor_methods.push(quote! {
                pub fn #setter_ident(&self, val: #rust_ty) {
                    static PROP: std::sync::OnceLock<::uika::ffi::FPropertyHandle> = std::sync::OnceLock::new();
                    let prop = *PROP.get_or_init(|| unsafe {
                        ((*::uika::runtime::api().reflection).find_property)(
                            <Self as ::uika::runtime::UeClass>::static_class(),
                            [#(#ue_name_bytes),*].as_ptr(),
                            #ue_name_len,
                        )
                    });
                    unsafe { ((*::uika::runtime::api().property).#setter_fn)(self.__obj, prop, val); }
                }
            });
        }
    }

    // Rust private field accessors
    for f in &rust_fields {
        let ident = &f.ident;
        let ty = &f.ty;
        let setter_ident = format_ident!("set_{}", ident);

        accessor_methods.push(quote! {
            pub fn #ident(&self) -> #ty {
                unsafe { (*self.__rust_data).#ident }
            }
            pub fn #setter_ident(&mut self, val: #ty) {
                unsafe { (*self.__rust_data).#ident = val; }
            }
        });
    }

    // Component accessors (find default subobject by name)
    for comp in &components {
        let field_ident = &comp.ident;
        let comp_type = &comp.component_type;
        let comp_name = prop_type::to_pascal_case(&field_ident.to_string());
        let comp_name_bytes = comp_name.as_bytes();
        let comp_name_len = comp_name.len() as u32;

        accessor_methods.push(quote! {
            pub fn #field_ident(&self) -> ::uika::runtime::UikaResult<::uika::runtime::UObjectRef<#comp_type>> {
                let h = unsafe {
                    ((*::uika::runtime::api().reify).find_default_subobject)(
                        self.__obj,
                        [#(#comp_name_bytes),*].as_ptr(), #comp_name_len,
                    )
                };
                if h.0.is_null() {
                    return Err(::uika::runtime::UikaError::InvalidOperation(
                        concat!("subobject '", #comp_name, "' not found").into()
                    ));
                }
                Ok(unsafe { ::uika::runtime::UObjectRef::from_raw(h) })
            }
        });
    }

    // from_obj: cast from UObjectRef<impl UeClass> to the reified struct
    accessor_methods.push(quote! {
        /// Cast a `UObjectRef` to this reified type.
        ///
        /// Checks `IsA` and retrieves the Rust-side instance data.
        /// Fails if the object is not an instance of this class or has no Rust data.
        pub fn from_obj(obj: ::uika::runtime::UObjectRef<impl ::uika::runtime::UeClass>) -> ::uika::runtime::UikaResult<Self> {
            let handle = obj.checked()?.raw();
            let is_a = unsafe {
                ((*::uika::runtime::api().core).is_a)(handle, <Self as ::uika::runtime::UeClass>::static_class())
            };
            if !is_a {
                return Err(::uika::runtime::UikaError::InvalidCast);
            }
            let rust_data = ::uika::runtime::reify_registry::get_instance_data(handle)
                as *mut #rust_data_name;
            if rust_data.is_null() {
                return Err(::uika::runtime::UikaError::InvalidOperation("no rust data for reified cast".into()));
            }
            Ok(Self { __obj: handle, __rust_data: rust_data })
        }
    });

    let accessors_impl = quote! {
        impl #struct_name {
            #(#accessor_methods)*
        }
    };

    // --- 6. Registration function ---
    let parent_path = &args.parent_path;
    let parent_name = args.parent_name.as_str();
    let parent_name_bytes = parent_name.as_bytes();
    let parent_name_len = parent_name.len() as u32;
    let struct_name_bytes = struct_name_str.as_bytes();
    let struct_name_byte_len = struct_name_str.len() as u32;

    // Generate add_property calls
    let mut add_prop_stmts: Vec<TokenStream> = Vec::new();
    let mut cdo_default_stmts: Vec<TokenStream> = Vec::new();

    for prop in &uprops {
        let info = prop_type::map_type(&prop.ty).unwrap();
        let ue_name = prop_type::to_pascal_case(&prop.ident.to_string());
        let ue_name_bytes = ue_name.as_bytes();
        let ue_name_len = ue_name.len() as u32;
        let prop_type_expr = &info.prop_type_expr;
        let prop_var = format_ident!("_prop_{}", prop.ident);

        // Compute flags:
        //   BlueprintReadWrite → visible + editable in Details and Blueprint
        //   BlueprintReadOnly  → visible in Blueprint (get only) + visible but greyed in Details
        //   EditAnywhere       → editable in Details
        let mut flag_parts = Vec::new();
        if prop.args.blueprint_read_write || prop.args.blueprint_read_only {
            flag_parts.push(quote! { ::uika::ffi::CPF_BLUEPRINT_VISIBLE });
        }
        if prop.args.blueprint_read_only {
            flag_parts.push(quote! { ::uika::ffi::CPF_BLUEPRINT_READ_ONLY });
            // VisibleAnywhere: show in Details as read-only
            flag_parts.push(quote! { ::uika::ffi::CPF_EDIT });
            flag_parts.push(quote! { ::uika::ffi::CPF_EDIT_CONST });
        }
        if prop.args.edit_anywhere || prop.args.blueprint_read_write {
            flag_parts.push(quote! { ::uika::ffi::CPF_EDIT });
        }
        if flag_parts.is_empty() {
            flag_parts.push(quote! { 0u64 });
        }
        let flags_expr = quote! { #(#flag_parts)|* };

        add_prop_stmts.push(quote! {
            let #prop_var = unsafe {
                (reify.add_property)(
                    class,
                    [#(#ue_name_bytes),*].as_ptr(),
                    #ue_name_len,
                    #prop_type_expr as u32,
                    #flags_expr,
                    std::ptr::null(),
                )
            };
        });

        // CDO default
        if let Some(ref default_expr) = prop.args.default_expr {
            let setter_fn = &info.setter_fn;
            cdo_default_stmts.push(quote! {
                if !#prop_var.0.is_null() {
                    unsafe { ((*table.property).#setter_fn)(cdo, #prop_var, #default_expr); }
                }
            });
        }
    }

    // Generate add_default_subobject calls
    let mut add_comp_stmts: Vec<TokenStream> = Vec::new();
    for comp in &components {
        let comp_type = &comp.component_type;
        let comp_name = prop_type::to_pascal_case(&comp.ident.to_string());
        let comp_name_bytes = comp_name.as_bytes();
        let comp_name_len = comp_name.len() as u32;

        let mut flags: u32 = 0;
        if comp.is_root {
            flags |= 1; // UIKA_COMP_ROOT
        }

        let (attach_ptr, attach_len) = if let Some(ref attach_name) = comp.attach_to {
            let attach_pascal = prop_type::to_pascal_case(attach_name);
            let attach_bytes = attach_pascal.as_bytes().to_vec();
            let alen = attach_bytes.len() as u32;
            (quote! { [#(#attach_bytes),*].as_ptr() }, quote! { #alen })
        } else {
            (quote! { std::ptr::null() }, quote! { 0u32 })
        };

        add_comp_stmts.push(quote! {
            {
                let comp_class = <#comp_type as ::uika::runtime::UeClass>::static_class();
                unsafe {
                    (reify.add_default_subobject)(
                        class,
                        [#(#comp_name_bytes),*].as_ptr(), #comp_name_len,
                        comp_class,
                        #flags,
                        #attach_ptr, #attach_len,
                    );
                }
            }
        });
    }

    // Generate CDO default stmts for finalize (re-find properties by name)
    let mut finalize_cdo_stmts: Vec<TokenStream> = Vec::new();
    for prop in &uprops {
        if let Some(ref default_expr) = prop.args.default_expr {
            let info = prop_type::map_type(&prop.ty).unwrap();
            let ue_name = prop_type::to_pascal_case(&prop.ident.to_string());
            let ue_name_bytes = ue_name.as_bytes();
            let ue_name_len = ue_name.len() as u32;
            let setter_fn = &info.setter_fn;

            finalize_cdo_stmts.push(quote! {
                {
                    let prop = unsafe {
                        ((*table.reflection).find_property)(
                            class,
                            [#(#ue_name_bytes),*].as_ptr(),
                            #ue_name_len,
                        )
                    };
                    if !prop.0.is_null() {
                        unsafe { ((*table.property).#setter_fn)(cdo, prop, #default_expr); }
                    }
                }
            });
        }
    }

    let register_fn = quote! {
        #[doc(hidden)]
        pub fn #register_fn_name(table: &::uika::ffi::UikaApiTable) {
            const TYPE_ID: u64 = #type_id_value;

            // Register Rust type info
            ::uika::runtime::reify_registry::register_type(
                TYPE_ID,
                ::uika::runtime::reify_registry::RustTypeInfo {
                    name: #struct_name_str,
                    construct_fn: || {
                        Box::into_raw(Box::new(#rust_data_name::default())) as *mut u8
                    },
                    drop_fn: |ptr| {
                        if !ptr.is_null() {
                            let _ = unsafe { Box::from_raw(ptr as *mut #rust_data_name) };
                        }
                    },
                },
            );

            let reify = unsafe { &*table.reify };

            // Find parent class
            let parent = unsafe {
                ((*table.reflection).find_class)(
                    [#(#parent_name_bytes),*].as_ptr(),
                    #parent_name_len,
                )
            };
            if parent.0.is_null() {
                if !table.logging.is_null() {
                    let msg = concat!("[Uika] ", stringify!(#struct_name), ": failed to find parent class '", #parent_name, "'");
                    let bytes = msg.as_bytes();
                    unsafe { ((*table.logging).log)(2, bytes.as_ptr(), bytes.len() as u32); }
                }
                return;
            }

            // Create class
            let class = unsafe {
                (reify.create_class)(
                    [#(#struct_name_bytes),*].as_ptr(),
                    #struct_name_byte_len,
                    parent,
                    TYPE_ID,
                )
            };
            if class.0.is_null() {
                if !table.logging.is_null() {
                    let msg = concat!("[Uika] ", stringify!(#struct_name), ": create_class failed");
                    let bytes = msg.as_bytes();
                    unsafe { ((*table.logging).log)(2, bytes.as_ptr(), bytes.len() as u32); }
                }
                return;
            }
            #class_handle_name.set(class).ok();

            // Add properties (finalize deferred to __uika_finalize)
            #(#add_prop_stmts)*

            // Register default subobjects
            #(#add_comp_stmts)*
        }
    };

    let finalize_fn = quote! {
        #[doc(hidden)]
        pub fn #finalize_fn_name(table: &::uika::ffi::UikaApiTable) {
            let class = match #class_handle_name.get() {
                Some(&c) if !c.0.is_null() => c,
                _ => return,
            };
            let reify = unsafe { &*table.reify };

            // Finalize
            let result = unsafe { (reify.finalize_class)(class) };
            if result != ::uika::ffi::UikaErrorCode::Ok {
                if !table.logging.is_null() {
                    let msg = concat!("[Uika] ", stringify!(#struct_name), ": finalize_class failed");
                    let bytes = msg.as_bytes();
                    unsafe { ((*table.logging).log)(2, bytes.as_ptr(), bytes.len() as u32); }
                }
                return;
            }

            // Set CDO defaults
            let cdo = unsafe { (reify.get_cdo)(class) };
            if !cdo.0.is_null() {
                #(#finalize_cdo_stmts)*
            }

            if !table.logging.is_null() {
                let msg = concat!("[Uika] ", stringify!(#struct_name), " registered successfully");
                let bytes = msg.as_bytes();
                unsafe { ((*table.logging).log)(0, bytes.as_ptr(), bytes.len() as u32); }
            }
        }
    };

    // --- Compile-time parent type check ---
    let comp_type_checks: Vec<TokenStream> = components
        .iter()
        .map(|c| {
            let ct = &c.component_type;
            quote! { _assert_ue_class::<#ct>(); }
        })
        .collect();

    let parent_check = quote! {
        const _: () = {
            fn _uika_parent_check() {
                fn _assert_ue_class<T: ::uika::runtime::UeClass>() {}
                _assert_ue_class::<#parent_path>();
                #(#comp_type_checks)*
            }
        };
    };

    // --- Deref to UObjectRef<Parent> for auto-deref to parent Ext trait methods ---
    let deref_impl = quote! {
        impl std::ops::Deref for #struct_name {
            type Target = ::uika::runtime::UObjectRef<#parent_path>;
            fn deref(&self) -> &::uika::runtime::UObjectRef<#parent_path> {
                // SAFETY: UObjectRef<T> is repr(transparent) over UObjectHandle,
                // and __obj is the first field of the reified struct.
                unsafe { &*(&self.__obj as *const ::uika::ffi::UObjectHandle as *const ::uika::runtime::UObjectRef<#parent_path>) }
            }
        }
    };

    // Combine everything
    Ok(quote! {
        #user_struct
        #rust_data_struct
        #static_handle
        #ue_class_impl
        #parent_check
        #deref_impl
        #accessors_impl
        #register_fn
        #finalize_fn

        ::uika::__inventory::submit! {
            ::uika::runtime::reify_registry::ClassRegistration {
                register: #register_fn_name,
                finalize: #finalize_fn_name,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

pub(crate) fn to_screaming_snake(s: &str) -> String {
    to_snake_case(s).to_uppercase()
}
