// Shared property codegen for both classes and structs.
//
// Property generation (getters/setters via the reflection PropertyApi) is
// identical for UClass and UScriptStruct except for:
// - The reflection lookup function (find_property vs find_struct_property)
// - The handle expression (static_class() vs static_struct())
// - The handle access (classes do self.handle(), structs don't)
// - The container expression passed to the PropertyApi (h vs self.as_ptr())
//
// `PropertyContext` captures these differences so all the codegen helpers
// can be shared.

use crate::context::CodegenContext;
use crate::naming::{strip_bool_prefix, to_snake_case};
use crate::schema::PropertyInfo;
use crate::type_map::{self, ConversionKind, MappedType};

/// Context that parameterizes property codegen for classes vs structs.
pub struct PropertyContext {
    /// Reflection API function name: "find_property" or "find_struct_property".
    pub find_prop_fn: String,
    /// Expression to get the type handle for property lookup.
    /// e.g., "Actor::static_class()" or "FVector::static_struct()".
    pub handle_expr: String,
    /// Validity check statement (including semicolon). Empty string for structs.
    /// e.g., "let h = self.handle();"
    pub pre_access: String,
    /// Container expression for property API calls: "h" or "self.as_ptr()".
    pub container_expr: String,
    /// Whether this is a UClass context (true) or struct context (false).
    /// Container properties are only valid in class contexts.
    pub is_class: bool,
}

/// Collect supported, deduplicated properties, returning their getter name set and the property list.
/// Properties referencing types not in the context (e.g., enums/classes from non-enabled modules)
/// are filtered out.
pub fn collect_deduped_properties<'a>(
    props: &'a [PropertyInfo],
    ctx: Option<&CodegenContext>,
) -> (std::collections::HashSet<String>, Vec<&'a PropertyInfo>) {
    let mut prop_names = std::collections::HashSet::new();
    let mut deduped = Vec::new();

    for prop in props {
        // Skip BlueprintGetter/Setter properties (accessed via function calls)
        if prop.getter.is_some() || prop.setter.is_some() {
            continue;
        }

        let mapped = type_map::map_property_type(
            &prop.prop_type,
            prop.class_name.as_deref(),
            prop.struct_name.as_deref(),
            prop.enum_name.as_deref(),
            prop.enum_underlying_type.as_deref(),
            prop.meta_class_name.as_deref(),
            prop.interface_name.as_deref(),
        );
        if !mapped.supported {
            continue;
        }

        // ObjectRef: check that the referenced class/interface is available
        if matches!(mapped.rust_to_ffi, ConversionKind::ObjectRef) {
            if let Some(ctx) = ctx {
                let type_available = match prop.prop_type.as_str() {
                    "ClassProperty" => {
                        let effective = prop.meta_class_name.as_deref().or(prop.class_name.as_deref());
                        effective.map_or(true, |c| ctx.classes.contains_key(c))
                    }
                    "InterfaceProperty" => {
                        prop.interface_name.as_deref().map_or(false, |i| ctx.classes.contains_key(i))
                    }
                    _ => prop.class_name.as_deref().map_or(true, |c| ctx.classes.contains_key(c)),
                };
                if !type_available {
                    continue;
                }
            }
        }

        // StructOpaque: only allow if the struct is in enabled modules with static_struct
        if matches!(mapped.rust_to_ffi, ConversionKind::StructOpaque) {
            let valid = if let Some(ctx) = ctx {
                prop.struct_name.as_deref().map_or(false, |sn| {
                    ctx.structs.get(sn).map_or(false, |si| si.has_static_struct)
                })
            } else {
                prop.struct_name.is_some()
            };
            if !valid {
                continue;
            }
        }

        // Container types: skip if inner types can't be resolved
        if matches!(
            mapped.rust_to_ffi,
            ConversionKind::ContainerArray | ConversionKind::ContainerMap | ConversionKind::ContainerSet
        ) {
            if type_map::resolve_container_rust_type(prop, ctx).is_none() {
                continue;
            }
            // Container properties only work on UClass contexts (need UObject owner),
            // not on struct contexts. `ctx` is None only for non-class callers.
            // More precisely: skip containers in struct property contexts.
        }

        // Skip properties that reference types not in enabled modules
        if let Some(ctx) = ctx {
            match mapped.rust_to_ffi {
                ConversionKind::EnumCast => {
                    if let Some(en) = &prop.enum_name {
                        if !ctx.enums.contains_key(en.as_str()) {
                            continue;
                        }
                    }
                }
                ConversionKind::ObjectRef => {
                    if let Some(cn) = &prop.class_name {
                        if !ctx.classes.contains_key(cn.as_str()) {
                            continue;
                        }
                    }
                }
                _ => {}
            }
        }

        let rust_name = if prop.prop_type == "BoolProperty" {
            strip_bool_prefix(&prop.name)
        } else {
            to_snake_case(&prop.name)
        };

        // Container and delegate properties use bare name (no get_/set_ prefix)
        let is_container = matches!(
            mapped.rust_to_ffi,
            ConversionKind::ContainerArray | ConversionKind::ContainerMap | ConversionKind::ContainerSet
        );
        let is_delegate = matches!(
            mapped.rust_to_ffi,
            ConversionKind::Delegate | ConversionKind::MulticastDelegate
        );
        let getter_name = if is_container || is_delegate {
            rust_name.clone()
        } else {
            format!("get_{rust_name}")
        };
        if prop_names.contains(&getter_name) {
            continue;
        }
        prop_names.insert(getter_name);
        if !is_container && !is_delegate {
            prop_names.insert(format!("set_{rust_name}"));
        }
        deduped.push(prop);
    }

    (prop_names, deduped)
}

/// Generate a getter and setter for a single property (used as default impls in Ext trait).
///
/// Setters whose name appears in `suppress_setters` are skipped.
pub fn generate_property(
    out: &mut String,
    prop: &PropertyInfo,
    pctx: &PropertyContext,
    ctx: &CodegenContext,
    suppress_setters: &std::collections::HashSet<String>,
) {
    let mapped = type_map::map_property_type(
        &prop.prop_type,
        prop.class_name.as_deref(),
        prop.struct_name.as_deref(),
        prop.enum_name.as_deref(),
        prop.enum_underlying_type.as_deref(),
        prop.meta_class_name.as_deref(),
        prop.interface_name.as_deref(),
    );
    if !mapped.supported {
        return;
    }

    // Delegate properties: handled by delegates.rs, skip here.
    if matches!(
        mapped.rust_to_ffi,
        ConversionKind::Delegate | ConversionKind::MulticastDelegate
    ) {
        return;
    }

    let prop_name = &prop.name;
    let rust_name = if prop.prop_type == "BoolProperty" {
        strip_bool_prefix(prop_name)
    } else {
        to_snake_case(prop_name)
    };
    let prop_name_len = prop_name.len();
    let byte_lit = format!("b\"{}\\0\"", prop_name);

    // Fixed array properties: use indexed access via get_property_at/set_property_at
    if prop.array_dim > 1 {
        generate_fixed_array_property(out, prop, &rust_name, &byte_lit, prop_name_len, pctx, ctx, &mapped);
        return;
    }

    // Container types: generate handle-returning getter (no setter)
    // Only valid in class contexts (containers need a UObject owner)
    if matches!(
        mapped.rust_to_ffi,
        ConversionKind::ContainerArray | ConversionKind::ContainerMap | ConversionKind::ContainerSet
    ) {
        if pctx.is_class {
            if let Some(container_type) = type_map::resolve_container_rust_type(prop, Some(ctx)) {
                generate_container_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &container_type);
            }
        }
        return;
    }

    // Getter
    match mapped.rust_to_ffi {
        ConversionKind::StringUtf8 => {
            generate_string_getter(out, &rust_name, &byte_lit, prop_name_len, pctx);
        }
        ConversionKind::StructOpaque => {
            let struct_cpp = prop.struct_name.as_deref()
                .and_then(|sn| ctx.structs.get(sn))
                .map(|si| si.cpp_name.clone())
                .unwrap_or_else(|| format!("F{}", prop.struct_name.as_deref().unwrap_or("Unknown")));
            generate_struct_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &struct_cpp);
            let setter_name = format!("set_{rust_name}");
            if !suppress_setters.contains(&setter_name) {
                generate_struct_setter(out, &rust_name, &byte_lit, prop_name_len, pctx, &struct_cpp);
            }
            return;
        }
        ConversionKind::ObjectRef => {
            generate_object_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
        ConversionKind::EnumCast => {
            let actual_repr = prop
                .enum_name
                .as_deref()
                .and_then(|en| ctx.enum_actual_repr(en))
                .unwrap_or(&mapped.rust_ffi_type);
            generate_enum_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped, actual_repr);
        }
        ConversionKind::FName => {
            generate_fname_getter(out, &rust_name, &byte_lit, prop_name_len, pctx);
        }
        ConversionKind::IntCast => {
            generate_int_cast_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
        _ => {
            generate_primitive_getter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
    }

    // Setter (suppressed when a UFUNCTION with the same name takes priority)
    let setter_name = format!("set_{rust_name}");
    if suppress_setters.contains(&setter_name) {
        return;
    }
    match mapped.rust_to_ffi {
        ConversionKind::StringUtf8 => {
            generate_string_setter(out, &rust_name, &byte_lit, prop_name_len, pctx);
        }
        ConversionKind::StructOpaque => { /* handled in getter branch with early return */ }
        ConversionKind::ObjectRef => {
            generate_object_setter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
        ConversionKind::EnumCast => {
            generate_enum_setter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
        ConversionKind::FName => {
            generate_fname_setter(out, &rust_name, &byte_lit, prop_name_len, pctx);
        }
        ConversionKind::IntCast => {
            generate_int_cast_setter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
        _ => {
            generate_primitive_setter(out, &rust_name, &byte_lit, prop_name_len, pctx, &mapped);
        }
    }
}

/// Get a default value literal for a Rust type.
pub fn default_value_for(rust_type: &str) -> &'static str {
    match rust_type {
        "bool" => "false",
        "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" => "0",
        "f32" => "0.0f32",
        "f64" => "0.0f64",
        _ => "Default::default()",
    }
}

// ---------------------------------------------------------------------------
// Prop lookup boilerplate
// ---------------------------------------------------------------------------

/// Generates the property OnceLock lookup. Returns after the closing `});`.
fn emit_prop_lookup(out: &mut String, byte_lit: &str, prop_name_len: usize, pctx: &PropertyContext) {
    let find = &pctx.find_prop_fn;
    let handle = &pctx.handle_expr;
    out.push_str(&format!(
        "        static PROP: std::sync::OnceLock<uika_runtime::FPropertyHandle> = std::sync::OnceLock::new();\n\
         \x20       let prop = *PROP.get_or_init(|| unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::reflection_{find}(\n\
         \x20               {handle}, {byte_lit}.as_ptr(), {prop_name_len}\n\
         \x20           )\n\
         \x20       }});\n"
    ));
}

/// Emit pre-access (validity check) if needed.
fn emit_pre_access(out: &mut String, pctx: &PropertyContext) {
    if !pctx.pre_access.is_empty() {
        out.push_str(&format!("        {}\n", pctx.pre_access));
    }
}

// ---------------------------------------------------------------------------
// Getters
// ---------------------------------------------------------------------------

fn generate_primitive_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let getter = &mapped.property_getter;
    let default = default_value_for(rust_type);
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> {rust_type} {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut out = {default};\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_{getter}({c}, prop, &mut out) }}, \"{rust_name}\");\n\
         \x20       out\n\
         \x20   }}\n\n"
    ));
}

fn generate_int_cast_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let ffi_type = &mapped.rust_ffi_type;
    let getter = &mapped.property_getter;
    let default = default_value_for(ffi_type);
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> {rust_type} {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut out = {default};\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_{getter}({c}, prop, &mut out) }}, \"{rust_name}\");\n\
         \x20       out as {rust_type}\n\
         \x20   }}\n\n"
    ));
}

fn generate_string_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> String {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut buf = vec![0u8; 512];\n\
         \x20       let mut out_len: u32 = 0;\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_get_string({c}, prop, buf.as_mut_ptr(), buf.len() as u32, &mut out_len)\n\
         \x20       }}, \"{rust_name}\");\n\
         \x20       buf.truncate(out_len as usize);\n\
         \x20       String::from_utf8_lossy(&buf).into_owned()\n\
         \x20   }}\n\n"
    ));
}

fn generate_object_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> {rust_type} {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut raw = uika_runtime::UObjectHandle::null();\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_get_object({c}, prop, &mut raw) }}, \"{rust_name}\");\n\
         \x20       unsafe {{ uika_runtime::UObjectRef::from_raw(raw) }}\n\
         \x20   }}\n\n"
    ));
}

fn generate_enum_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
    actual_repr: &str,
) {
    let rust_type = &mapped.rust_type;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> {rust_type} {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut raw: i64 = 0;\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_get_enum({c}, prop, &mut raw) }}, \"{rust_name}\");\n\
         \x20       {rust_type}::from_value(raw as {actual_repr}).expect(\"unknown enum value\")\n\
         \x20   }}\n\n"
    ));
}

fn generate_fname_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> uika_runtime::FNameHandle {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let mut out = uika_runtime::FNameHandle(0);\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_get_fname({c}, prop, &mut out) }}, \"{rust_name}\");\n\
         \x20       out\n\
         \x20   }}\n\n"
    ));
}

// ---------------------------------------------------------------------------
// Setters
// ---------------------------------------------------------------------------

fn generate_primitive_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let setter = &mapped.property_setter;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: {rust_type}) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_{setter}({c}, prop, val) }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

fn generate_int_cast_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let ffi_type = &mapped.rust_ffi_type;
    let setter = &mapped.property_setter;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: {rust_type}) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_{setter}({c}, prop, val as {ffi_type}) }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

fn generate_string_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: &str) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_set_string({c}, prop, val.as_ptr(), val.len() as u32)\n\
         \x20       }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

fn generate_object_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: {rust_type}) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_set_object({c}, prop, val.raw()) }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

fn generate_enum_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    mapped: &MappedType,
) {
    let rust_type = &mapped.rust_type;
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: {rust_type}) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_set_enum({c}, prop, val as i64) }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

fn generate_fname_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: uika_runtime::FNameHandle) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        uika_runtime::ffi_infallible_ctx(unsafe {{ uika_runtime::ffi_dispatch::property_set_fname({c}, prop, val) }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

// ---------------------------------------------------------------------------
// Struct getters/setters (OwnedStruct<T>)
// ---------------------------------------------------------------------------

fn generate_struct_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    struct_cpp: &str,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn get_{rust_name}(&self) -> uika_runtime::OwnedStruct<{struct_cpp}> {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let size = unsafe {{ uika_runtime::ffi_dispatch::reflection_get_property_size(prop) }} as usize;\n\
         \x20       let mut buf = vec![0u8; size];\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_get_struct({c}, prop, buf.as_mut_ptr(), size as u32)\n\
         \x20       }}, \"{rust_name}\");\n\
         \x20       uika_runtime::OwnedStruct::from_bytes(buf)\n\
         \x20   }}\n\n"
    ));
}

fn generate_struct_setter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    struct_cpp: &str,
) {
    let c = &pctx.container_expr;

    out.push_str(&format!(
        "    fn set_{rust_name}(&self, val: &uika_runtime::OwnedStruct<{struct_cpp}>) {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        let __bytes = val.to_bytes();\n\
         \x20       uika_runtime::ffi_infallible_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_set_struct({c}, prop, __bytes.as_ptr(), __bytes.len() as u32)\n\
         \x20       }}, \"{rust_name}\");\n\
         \x20   }}\n\n"
    ));
}

// ---------------------------------------------------------------------------
// Fixed array getter/setter (array_dim > 1)
// ---------------------------------------------------------------------------

fn generate_fixed_array_property(
    out: &mut String,
    prop: &PropertyInfo,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    ctx: &CodegenContext,
    mapped: &MappedType,
) {
    let array_dim = prop.array_dim;
    let c = &pctx.container_expr;

    // Determine the Rust return type and conversion for the getter
    let (getter_ret_type, getter_conversion, setter_param_type, setter_conversion) = match mapped.rust_to_ffi {
        ConversionKind::Identity if mapped.rust_type == "bool" => (
            "bool".to_string(),
            "Ok(buf[0] != 0)".to_string(),
            "bool".to_string(),
            format!(
                "let mut buf = vec![0u8; elem_size];\n\
                 \x20       if val {{ buf[0] = 1; }}"
            ),
        ),
        ConversionKind::Identity | ConversionKind::IntCast => {
            let rust_type = &mapped.rust_type;
            let byte_count = rust_type_byte_size(rust_type);
            (
                rust_type.clone(),
                format!("Ok({rust_type}::from_ne_bytes(buf[..{byte_count}].try_into().unwrap()))"),
                rust_type.clone(),
                format!("let buf = val.to_ne_bytes().to_vec();"),
            )
        }
        ConversionKind::ObjectRef => {
            let rust_type = &mapped.rust_type;
            (
                rust_type.clone(),
                "let handle = uika_runtime::UObjectHandle::from_addr(u64::from_ne_bytes(buf[..8].try_into().unwrap()));\n\
                 \x20       Ok(unsafe { uika_runtime::UObjectRef::from_raw(handle) })".to_string(),
                rust_type.clone(),
                "let buf = val.raw().to_addr().to_ne_bytes().to_vec();".to_string(),
            )
        }
        ConversionKind::EnumCast => {
            let rust_type = &mapped.rust_type;
            let actual_repr = prop
                .enum_name
                .as_deref()
                .and_then(|en| ctx.enum_actual_repr(en))
                .unwrap_or(&mapped.rust_ffi_type);
            let byte_count = rust_type_byte_size(actual_repr);
            (
                rust_type.clone(),
                format!(
                    "let raw = {actual_repr}::from_ne_bytes(buf[..{byte_count}].try_into().unwrap());\n\
                     \x20       {rust_type}::from_value(raw).ok_or(uika_runtime::UikaError::TypeMismatch)"
                ),
                rust_type.clone(),
                format!("let buf = (val as {actual_repr}).to_ne_bytes().to_vec();"),
            )
        }
        ConversionKind::StructOpaque => {
            let struct_cpp = prop.struct_name.as_deref()
                .and_then(|sn| ctx.structs.get(sn))
                .map(|si| si.cpp_name.clone())
                .unwrap_or_else(|| format!("F{}", prop.struct_name.as_deref().unwrap_or("Unknown")));
            (
                format!("uika_runtime::OwnedStruct<{struct_cpp}>"),
                "Ok(uika_runtime::OwnedStruct::from_bytes(buf))".to_string(),
                format!("&uika_runtime::OwnedStruct<{struct_cpp}>"),
                "let buf = val.to_bytes();".to_string(),
            )
        }
        _ => return, // Unsupported conversion kind for fixed arrays
    };

    // Getter
    out.push_str(&format!(
        "    fn get_{rust_name}(&self, index: u32) -> uika_runtime::UikaResult<{getter_ret_type}> {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        if index >= {array_dim} {{ return Err(uika_runtime::UikaError::IndexOutOfRange); }}\n\
         \x20       let elem_size = unsafe {{ uika_runtime::ffi_dispatch::reflection_get_element_size(prop) }} as usize;\n\
         \x20       let mut buf = vec![0u8; elem_size];\n\
         \x20       uika_runtime::check_ffi_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_get_property_at({c}, prop, index, buf.as_mut_ptr(), elem_size as u32)\n\
         \x20       }}, \"{rust_name}\")?;\n\
         \x20       {getter_conversion}\n\
         \x20   }}\n\n"
    ));

    // Setter
    out.push_str(&format!(
        "    fn set_{rust_name}(&self, index: u32, val: {setter_param_type}) -> uika_runtime::UikaResult<()> {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    out.push_str(&format!(
        "        if index >= {array_dim} {{ return Err(uika_runtime::UikaError::IndexOutOfRange); }}\n\
         \x20       let elem_size = unsafe {{ uika_runtime::ffi_dispatch::reflection_get_element_size(prop) }} as usize;\n\
         \x20       {setter_conversion}\n\
         \x20       uika_runtime::check_ffi_ctx(unsafe {{\n\
         \x20           uika_runtime::ffi_dispatch::property_set_property_at({c}, prop, index, buf.as_ptr(), elem_size as u32)\n\
         \x20       }}, \"{rust_name}\")?;\n\
         \x20       Ok(())\n\
         \x20   }}\n\n"
    ));
}

/// Get the byte size of a Rust primitive type for from_ne_bytes conversions.
fn rust_type_byte_size(rust_type: &str) -> usize {
    match rust_type {
        "bool" | "i8" | "u8" => 1,
        "i16" | "u16" => 2,
        "i32" | "u32" | "f32" => 4,
        "i64" | "u64" | "f64" => 8,
        _ => 8, // fallback for pointer-sized types
    }
}

// ---------------------------------------------------------------------------
// Container getter
// ---------------------------------------------------------------------------

fn generate_container_getter(
    out: &mut String,
    rust_name: &str,
    byte_lit: &str,
    prop_name_len: usize,
    pctx: &PropertyContext,
    container_type: &str,
) {
    out.push_str(&format!(
        "    fn {rust_name}(&self) -> {container_type} {{\n"
    ));
    emit_prop_lookup(out, byte_lit, prop_name_len, pctx);
    emit_pre_access(out, pctx);
    // Container getter returns a handle (not data), constructed from the owner + prop handles.
    // Use turbofish syntax (::< >) because the type is in expression position.
    let turbofish_type = container_type.replace('<', "::<");
    out.push_str(&format!(
        "        {turbofish_type}::new(h, prop)\n\
         \x20   }}\n\n"
    ));
}
