// Rust class generation: marker types, UeClass trait, properties, functions.

use std::collections::HashSet;

use crate::context::{CodegenContext, FuncEntry};
use crate::defaults;
use crate::naming::{escape_reserved, to_snake_case};
use crate::schema::*;
use crate::type_map::{self, ConversionKind, MappedType, ParamDirection};

use super::delegates;
use super::properties::{self, PropertyContext};

/// Generate Rust code for a single UE class.
pub fn generate_class(class: &ClassInfo, ctx: &CodegenContext) -> String {
    let mut out = String::with_capacity(8192);

    // Import traits and types from own module and all other enabled modules
    out.push_str("use super::*;\n");
    out.push_str("use uika_runtime::{UeClass, UeStruct, UeEnum, ValidHandle, Pinned, Checked};\n");
    let current_module = ctx.package_to_module.get(&class.package).map(|s| s.as_str()).unwrap_or("");
    for module in &ctx.enabled_modules {
        if module != current_module {
            if let Some(feature) = ctx.feature_for_module(module) {
                out.push_str(&format!("#[cfg(feature = \"{feature}\")]\n"));
            }
            out.push_str(&format!("use crate::{module}::*;\n"));
        }
    }
    out.push('\n');

    let name = &class.name;         // e.g., "Actor"
    let cpp_name = &class.cpp_name; // e.g., "AActor"

    // Use the JSON `name` as the Rust struct name.
    // This keeps it consistent with UE naming (Actor, Pawn, etc.)
    out.push_str(&format!(
        "/// UE class `{cpp_name}`.\n\
         pub struct {name};\n\n"
    ));

    // UeClass trait impl
    let name_bytes_len = name.len();
    let byte_lit = format!("b\"{}\\0\"", name);
    out.push_str(&format!(
        "impl uika_runtime::UeClass for {name} {{\n\
         \x20   fn static_class() -> uika_runtime::UClassHandle {{\n\
         \x20       static CACHE: std::sync::OnceLock<uika_runtime::UClassHandle> = std::sync::OnceLock::new();\n\
         \x20       *CACHE.get_or_init(|| unsafe {{\n\
         \x20           ((*uika_runtime::api().reflection).get_static_class)({byte_lit}.as_ptr(), {name_bytes_len})\n\
         \x20       }})\n\
         \x20   }}\n\
         }}\n\n"
    ));

    // HasParent impl (must come before early-return — a class with no own
    // members still needs HasParent for the Deref chain)
    if let Some(parent) = &class.super_class {
        if ctx.classes.contains_key(parent.as_str()) {
            // Cfg-gate if parent is in a different module
            let parent_class = ctx.classes.get(parent.as_str()).unwrap();
            let parent_module = ctx.package_to_module.get(&parent_class.package)
                .map(|s| s.as_str()).unwrap_or("");
            if parent_module != current_module {
                if let Some(feature) = ctx.feature_for_module(parent_module) {
                    out.push_str(&format!("#[cfg(feature = \"{feature}\")]\n"));
                }
            }
            out.push_str(&format!(
                "impl uika_runtime::HasParent for {name} {{\n\
                 \x20   type Parent = {parent};\n\
                 }}\n\n"
            ));
        }
    }

    // Collect own functions only (inherited methods are accessed via Deref chain)
    let mut seen_func_names: HashSet<String> = HashSet::new();
    let mut class_funcs: Vec<&FuncEntry> = Vec::new();
    for entry in ctx.func_table.iter().filter(|e| e.class_name == *name) {
        if seen_func_names.insert(entry.rust_func_name.clone()) {
            class_funcs.push(entry);
        }
    }

    // Collect own property accessor names, deduplicating
    let (mut prop_names, deduped_props) = properties::collect_deduped_properties(&class.props, Some(ctx));

    // Collect own delegate properties
    let own_delegate_infos = delegates::collect_delegate_props(&class.props, name, ctx);

    // Only generate extension trait if there are own properties, functions, or delegates
    if deduped_props.is_empty() && class_funcs.is_empty()
        && own_delegate_infos.is_empty()
    {
        return out;
    }

    // Detect setter-function collisions: when a UFUNCTION matches a property setter name,
    // keep the UFUNCTION and suppress the setter (Option B from TODO_IMPROVEMENTS)
    let func_names: HashSet<String> = class_funcs
        .iter()
        .map(|e| escape_reserved(&e.rust_func_name))
        .collect();

    let suppress_setters: HashSet<String> = prop_names
        .iter()
        .filter(|n| n.starts_with("set_") && func_names.contains(n.as_str()))
        .cloned()
        .collect();

    // Remove suppressed setters from prop_names so they don't block UFUNCTIONs
    for setter in &suppress_setters {
        prop_names.remove(setter);
    }

    // Filter out functions whose names still collide with remaining property accessors
    let class_funcs: Vec<&FuncEntry> = class_funcs
        .into_iter()
        .filter(|e| !prop_names.contains(&escape_reserved(&e.rust_func_name)))
        .collect();

    // PropertyContext for class properties
    let pctx = PropertyContext {
        find_prop_fn: "find_property".to_string(),
        handle_expr: format!("{name}::static_class()"),
        pre_access: "let h = self.handle();".to_string(),
        container_expr: "h".to_string(),
        is_class: true,
    };

    // Generate delegate wrapper structs (own only)
    delegates::generate_delegate_structs(&mut out, &own_delegate_infos, name);

    // Extension trait with ValidHandle supertrait — default impls work for both
    // Checked<T> and Pinned<T> (dispatch via handle()).
    let trait_name = format!("{name}Ext");
    out.push_str(&format!(
        "pub trait {trait_name}: uika_runtime::ValidHandle {{\n"
    ));

    // Property getters/setters as default impls
    for prop in &deduped_props {
        properties::generate_property(&mut out, prop, &pctx, ctx, &suppress_setters);
    }

    // Delegate accessor default impls (own only)
    delegates::generate_delegate_impls(&mut out, &own_delegate_infos);

    // Function wrapper default impls
    for entry in &class_funcs {
        generate_function(&mut out, entry, &entry.class_name, ctx);
    }

    out.push_str("}\n\n");

    // Empty impls — Checked and Pinned both satisfy ValidHandle
    out.push_str(&format!(
        "impl {trait_name} for uika_runtime::Checked<{name}> {{}}\n"
    ));
    out.push_str(&format!(
        "impl {trait_name} for uika_runtime::Pinned<{name}> {{}}\n"
    ));

    out
}

// ---------------------------------------------------------------------------
// Container param helpers
// ---------------------------------------------------------------------------

fn is_container_param(param: &ParamInfo) -> bool {
    matches!(
        param.prop_type.as_str(),
        "ArrayProperty" | "MapProperty" | "SetProperty"
    )
}

/// Resolve the Rust input type for a container parameter (e.g., `&[Actor]`).
fn container_param_input_type(param: &ParamInfo, ctx: &CodegenContext) -> Option<String> {
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            let inner = param.inner_prop.as_ref()?;
            let elem = type_map::container_element_rust_type(inner, Some(ctx))?;
            Some(format!("&[{elem}]"))
        }
        "SetProperty" => {
            let elem = param.element_prop.as_ref()?;
            let etype = type_map::container_element_rust_type(elem, Some(ctx))?;
            Some(format!("&[{etype}]"))
        }
        "MapProperty" => {
            let key = param.key_prop.as_ref()?;
            let val = param.value_prop.as_ref()?;
            let kt = type_map::container_element_rust_type(key, Some(ctx))?;
            let vt = type_map::container_element_rust_type(val, Some(ctx))?;
            Some(format!("&[({kt}, {vt})]"))
        }
        _ => None,
    }
}

/// Resolve the Rust output type for a container parameter (e.g., `Vec<Actor>`).
fn container_param_output_type(param: &ParamInfo, ctx: &CodegenContext) -> Option<String> {
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            let inner = param.inner_prop.as_ref()?;
            let elem = type_map::container_element_rust_type(inner, Some(ctx))?;
            Some(format!("Vec<{elem}>"))
        }
        "SetProperty" => {
            let elem = param.element_prop.as_ref()?;
            let etype = type_map::container_element_rust_type(elem, Some(ctx))?;
            Some(format!("Vec<{etype}>"))
        }
        "MapProperty" => {
            let key = param.key_prop.as_ref()?;
            let val = param.value_prop.as_ref()?;
            let kt = type_map::container_element_rust_type(key, Some(ctx))?;
            let vt = type_map::container_element_rust_type(val, Some(ctx))?;
            Some(format!("Vec<({kt}, {vt})>"))
        }
        _ => None,
    }
}

/// Resolve the element type string for use in container type construction
/// (e.g., `UObjectRef<Actor>` for UeArray, or `K, V` for UeMap).
fn container_elem_type_str(param: &ParamInfo, ctx: &CodegenContext) -> Option<String> {
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            let inner = param.inner_prop.as_ref()?;
            type_map::container_element_rust_type(inner, Some(ctx))
        }
        "SetProperty" => {
            let elem = param.element_prop.as_ref()?;
            type_map::container_element_rust_type(elem, Some(ctx))
        }
        "MapProperty" => {
            let key = param.key_prop.as_ref()?;
            let val = param.value_prop.as_ref()?;
            let kt = type_map::container_element_rust_type(key, Some(ctx))?;
            let vt = type_map::container_element_rust_type(val, Some(ctx))?;
            Some(format!("{kt}, {vt}"))
        }
        _ => None,
    }
}

/// Build the composite return type from all output components.
fn build_return_type(output_types: &[String]) -> String {
    match output_types.len() {
        0 => "()".to_string(),
        1 => output_types[0].clone(),
        _ => format!("({})", output_types.join(", ")),
    }
}

/// Get the Rust type for a scalar Out/InOut param or ReturnValue in a return tuple.
/// StructOpaque returns `OwnedStruct<FStructName>` when the struct has UeStruct,
/// otherwise falls back to the raw pointer type.
fn scalar_out_rust_type_ctx(mapped: &MappedType, struct_name: Option<&str>, ctx: &CodegenContext) -> String {
    match mapped.ffi_to_rust {
        ConversionKind::StructOpaque => {
            if let Some(sn) = struct_name {
                if let Some(si) = ctx.structs.get(sn) {
                    if si.has_static_struct {
                        return format!("uika_runtime::OwnedStruct<{}>", si.cpp_name);
                    }
                }
            }
            // Struct not available or no static_struct — use raw pointer
            mapped.rust_type.clone()
        }
        _ => mapped.rust_type.clone(),
    }
}

/// Check if a StructOpaque return/out can use OwnedStruct (has valid UeStruct impl).
fn is_struct_owned(struct_name: Option<&str>, ctx: &CodegenContext) -> bool {
    struct_name.map_or(false, |sn| {
        ctx.structs.get(sn).map_or(false, |si| si.has_static_struct)
    })
}

/// Check if a scalar Out/InOut param should be included in the return tuple.
/// InOut StructOpaque params write back through the mutable pointer, so they
/// are NOT included in the return tuple.
fn is_scalar_output_returnable(dir: ParamDirection, mapped: &MappedType) -> bool {
    if dir == ParamDirection::InOut && mapped.ffi_to_rust == ConversionKind::StructOpaque {
        return false;
    }
    dir == ParamDirection::Out || dir == ParamDirection::InOut
}

// ---------------------------------------------------------------------------
// Function implementation (dispatch)
// ---------------------------------------------------------------------------

/// Generate a function wrapper (direct call via func_table).
fn generate_function(out: &mut String, entry: &FuncEntry, class_name: &str, ctx: &CodegenContext) {
    let has_container = entry.func.params.iter().any(|p| is_container_param(p));
    if has_container {
        generate_container_function(out, entry, class_name, ctx);
    } else {
        generate_scalar_function(out, entry, class_name, ctx);
    }
}

// ---------------------------------------------------------------------------
// Scalar function implementation (no container params — original path)
// ---------------------------------------------------------------------------

fn generate_scalar_function(out: &mut String, entry: &FuncEntry, _class_name: &str, ctx: &CodegenContext) {
    let func = &entry.func;
    let rust_fn_name = escape_reserved(&entry.rust_func_name);
    let func_id = entry.func_id;

    // Classify params
    let mut return_param: Option<&ParamInfo> = None;

    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Return {
            return_param = Some(param);
        }
    }

    // Map types for all params
    let mut all_mapped: Vec<(&ParamInfo, ParamDirection, MappedType)> = Vec::new();
    let mut all_supported = true;

    for param in &func.params {
        let dir = type_map::param_direction(param);
        let mapped = type_map::map_property_type(
            &param.prop_type,
            param.class_name.as_deref(),
            param.struct_name.as_deref(),
            param.enum_name.as_deref(),
            param.enum_underlying_type.as_deref(),
            param.meta_class_name.as_deref(),
            param.interface_name.as_deref(),
        );
        if !mapped.supported {
            all_supported = false;
            break;
        }
        all_mapped.push((param, dir, mapped));
    }

    if !all_supported {
        out.push_str(&format!(
            "    // Skipped: {} (unsupported param type)\n\n",
            func.name
        ));
        return;
    }

    // Determine return type
    let ret_mapped = return_param.map(|rp| {
        type_map::map_property_type(
            &rp.prop_type,
            rp.class_name.as_deref(),
            rp.struct_name.as_deref(),
            rp.enum_name.as_deref(),
            rp.enum_underlying_type.as_deref(),
            rp.meta_class_name.as_deref(),
            rp.interface_name.as_deref(),
        )
    });

    // Build return type: ReturnValue + all Out/InOut scalar params
    let return_rust_type = {
        let mut output_types = Vec::new();
        if let Some(m) = &ret_mapped {
            let rp_struct = return_param.and_then(|rp| rp.struct_name.as_deref());
            output_types.push(scalar_out_rust_type_ctx(m, rp_struct, ctx));
        }
        for (param, dir, mapped) in &all_mapped {
            if is_scalar_output_returnable(*dir, mapped) {
                output_types.push(scalar_out_rust_type_ctx(mapped, param.struct_name.as_deref(), ctx));
            }
        }
        build_return_type(&output_types)
    };

    // Build FFI type signature
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);

    // Build Rust function signature
    let mut sig = String::new();
    if is_static {
        sig.push_str(&format!(
            "    fn {rust_fn_name}("
        ));
    } else {
        sig.push_str(&format!(
            "    fn {rust_fn_name}(&self, "
        ));
    }

    // Input params
    let mut param_names = Vec::new();
    let mut default_unwraps: Vec<(String, String)> = Vec::new(); // (pname, default_expr)
    for (param, dir, mapped) in &all_mapped {
        if *dir == ParamDirection::Return {
            continue;
        }
        let pname = escape_reserved(&to_snake_case(&param.name));
        let has_default = *dir == ParamDirection::In
            && defaults::parse_default_literal(param, mapped, ctx).is_some();
        if has_default {
            let default_expr = defaults::parse_default_literal(param, mapped, ctx)
                .expect("default literal must be parseable (has_default was true)");
            default_unwraps.push((pname.clone(), default_expr));
        }
        match dir {
            ParamDirection::In | ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        if has_default {
                            sig.push_str(&format!("{pname}: Option<&str>, "));
                        } else {
                            sig.push_str(&format!("{pname}: &str, "));
                        }
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::In
                        && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                    {
                        let si = ctx.structs.get(param.struct_name.as_deref().expect("StructOpaque param must have struct_name"))
                            .expect("struct must exist in context");
                        sig.push_str(&format!(
                            "{pname}: &uika_runtime::OwnedStruct<{}>, ", si.cpp_name
                        ));
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::InOut => {
                        sig.push_str(&format!("{pname}: *mut u8, "));
                    }
                    _ => {
                        if has_default {
                            sig.push_str(&format!("{pname}: Option<{}>, ", mapped.rust_type));
                        } else {
                            sig.push_str(&format!("{pname}: {}, ", mapped.rust_type));
                        }
                    }
                }
            }
            ParamDirection::Out => {
                // Output params are returned as additional outputs — skip from signature for now
            }
            ParamDirection::Return => {}
        }
        param_names.push((pname, param, *dir, mapped));
    }

    // Remove trailing comma+space
    if sig.ends_with(", ") {
        sig.truncate(sig.len() - 2);
    }

    if return_rust_type == "()" {
        sig.push(')');
    } else {
        sig.push_str(&format!(") -> {return_rust_type}"));
    }

    out.push_str(&sig);
    out.push_str(" {\n");

    // Unwrap defaulted params before any FFI conversion
    for (pname, default_expr) in &default_unwraps {
        out.push_str(&format!("        let {pname} = {pname}.unwrap_or({default_expr});\n"));
    }

    // Build FFI fn type signature
    let mut ffi_params = String::new();
    if !is_static {
        ffi_params.push_str("uika_runtime::UObjectHandle, ");
    }
    for (_param, dir, mapped) in &all_mapped {
        match dir {
            ParamDirection::In | ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        ffi_params.push_str("*const u8, u32, ");
                        // InOut strings also have output buffer params
                        if *dir == ParamDirection::InOut {
                            ffi_params.push_str("*mut u8, u32, *mut u32, ");
                        }
                    }
                    ConversionKind::ObjectRef => {
                        ffi_params.push_str("uika_runtime::UObjectHandle, ");
                    }
                    ConversionKind::EnumCast => {
                        ffi_params.push_str(&format!("{}, ", mapped.rust_ffi_type));
                    }
                    ConversionKind::StructOpaque => {
                        if *dir == ParamDirection::InOut {
                            ffi_params.push_str("*mut u8, "); // mutable: data flows both ways
                        } else {
                            ffi_params.push_str("*const u8, ");
                        }
                    }
                    _ => {
                        ffi_params.push_str(&format!("{}, ", mapped.rust_ffi_type));
                    }
                }
            }
            ParamDirection::Out | ParamDirection::Return => {
                match mapped.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        ffi_params.push_str("*mut u8, u32, *mut u32, ");
                    }
                    ConversionKind::ObjectRef => {
                        ffi_params.push_str("*mut uika_runtime::UObjectHandle, ");
                    }
                    ConversionKind::StructOpaque => {
                        ffi_params.push_str("*mut u8, ");
                    }
                    ConversionKind::EnumCast => {
                        ffi_params.push_str(&format!("*mut {}, ", mapped.rust_ffi_type));
                    }
                    _ => {
                        ffi_params.push_str(&format!("*mut {}, ", mapped.rust_ffi_type));
                    }
                }
            }
        }
    }
    // Remove trailing comma+space
    if ffi_params.ends_with(", ") {
        ffi_params.truncate(ffi_params.len() - 2);
    }

    out.push_str(&format!(
        "        const FN_ID: u32 = {func_id};\n\
         \x20       type Fn = unsafe extern \"C\" fn({ffi_params}) -> uika_runtime::UikaErrorCode;\n\
         \x20       let __uika_fn: Fn = unsafe {{ std::mem::transmute(*(uika_runtime::api().func_table.add(FN_ID as usize))) }};\n"
    ));

    // Get handle for instance methods (pre-validated via ValidHandle)
    if !is_static {
        out.push_str("        let h = self.handle();\n");
    }

    // Declare output variables
    if let Some(_rp) = return_param {
        let rm = ret_mapped.as_ref().expect("return param must have mapped type");
        match rm.ffi_to_rust {
            ConversionKind::ObjectRef => {
                out.push_str("        let mut _ret = uika_runtime::UObjectHandle(std::ptr::null_mut());\n");
            }
            ConversionKind::StringUtf8 => {
                out.push_str("        let mut _ret_buf = vec![0u8; 512];\n");
                out.push_str("        let mut _ret_len: u32 = 0;\n");
            }
            ConversionKind::EnumCast => {
                out.push_str(&format!("        let mut _ret: {} = 0;\n", rm.rust_ffi_type));
            }
            ConversionKind::StructOpaque => {
                out.push_str("        let mut _ret_struct_buf = vec![0u8; 256];\n");
            }
            _ => {
                let default = properties::default_value_for(&rm.rust_ffi_type);
                out.push_str(&format!("        let mut _ret = {default};\n"));
            }
        }
    }

    for (param, dir, mapped) in &all_mapped {
        if *dir == ParamDirection::Out {
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::StructOpaque => {
                    out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 256];\n"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
                    out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
                }
                ConversionKind::ObjectRef => {
                    out.push_str(&format!("        let mut {pname} = uika_runtime::UObjectHandle(std::ptr::null_mut());\n"));
                }
                ConversionKind::EnumCast => {
                    out.push_str(&format!("        let mut {pname}: {} = 0;\n", mapped.rust_ffi_type));
                }
                _ => {
                    let default = properties::default_value_for(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let mut {pname} = {default};\n"));
                }
            }
        }
        // InOut string/text params need output buffers for the modified value
        if *dir == ParamDirection::InOut && mapped.ffi_to_rust == ConversionKind::StringUtf8 {
            let pname = escape_reserved(&to_snake_case(&param.name));
            out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
            out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
        }
    }

    // Build the FFI call (infallible after pre-validation)
    out.push_str("        uika_runtime::ffi_infallible(unsafe { __uika_fn(");
    if !is_static {
        out.push_str("h, ");
    }
    for (param, dir, mapped) in &all_mapped {
        let pname = escape_reserved(&to_snake_case(&param.name));
        match dir {
            ParamDirection::In | ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!("{pname}.as_ptr(), {pname}.len() as u32, "));
                        // InOut strings also pass output buffer params
                        if *dir == ParamDirection::InOut {
                            out.push_str(&format!("{pname}_buf.as_mut_ptr(), {pname}_buf.len() as u32, &mut {pname}_len, "));
                        }
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!("{pname}.raw(), "));
                    }
                    ConversionKind::EnumCast => {
                        out.push_str(&format!("{pname} as {}, ", mapped.rust_ffi_type));
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::In
                        && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                    {
                        out.push_str(&format!("{pname}.as_bytes().as_ptr(), "));
                    }
                    _ => {
                        out.push_str(&format!("{pname}, "));
                    }
                }
            }
            ParamDirection::Out => {
                match mapped.ffi_to_rust {
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!("{pname}_buf.as_mut_ptr(), "));
                    }
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!("{pname}_buf.as_mut_ptr(), {pname}_buf.len() as u32, &mut {pname}_len, "));
                    }
                    _ => {
                        out.push_str(&format!("&mut {pname}, "));
                    }
                }
            }
            ParamDirection::Return => {
                let rm = ret_mapped.as_ref().expect("return param must have mapped type");
                match rm.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str("_ret_buf.as_mut_ptr(), _ret_buf.len() as u32, &mut _ret_len, ");
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str("&mut _ret, ");
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str("_ret_struct_buf.as_mut_ptr(), ");
                    }
                    _ => {
                        out.push_str("&mut _ret, ");
                    }
                }
            }
        }
    }
    // Remove trailing comma+space in the call args
    let out_len = out.len();
    if out.ends_with(", ") {
        out.truncate(out_len - 2);
    }
    out.push_str(") });\n");

    // Return conversion: assemble ReturnValue + Out/InOut params (infallible)
    {
        let mut return_parts = Vec::new();

        // ReturnValue
        if return_param.is_some() {
            let rm = ret_mapped.as_ref().expect("return param must have mapped type");
            match rm.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    return_parts.push("unsafe { uika_runtime::UObjectRef::from_raw(_ret) }".to_string());
                }
                ConversionKind::StringUtf8 => {
                    out.push_str("        _ret_buf.truncate(_ret_len as usize);\n");
                    out.push_str("        let _ret_str = String::from_utf8_lossy(&_ret_buf).into_owned();\n");
                    return_parts.push("_ret_str".to_string());
                }
                ConversionKind::EnumCast => {
                    let rt = &rm.rust_type;
                    let rp = return_param.expect("return_param must be Some in return conversion");
                    let actual_repr = rp.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&rm.rust_ffi_type);
                    out.push_str(&format!("        let _ret_enum = {rt}::from_value(_ret as {actual_repr}).expect(\"unknown enum value\");\n"));
                    return_parts.push("_ret_enum".to_string());
                }
                ConversionKind::StructOpaque => {
                    let rp = return_param.expect("return_param must be Some in return conversion");
                    if is_struct_owned(rp.struct_name.as_deref(), ctx) {
                        out.push_str("        let _ret_owned = uika_runtime::OwnedStruct::from_bytes(_ret_struct_buf);\n");
                        return_parts.push("_ret_owned".to_string());
                    } else {
                        out.push_str("        let _ret_ptr = _ret_struct_buf.as_ptr();\n");
                        out.push_str("        std::mem::forget(_ret_struct_buf);\n");
                        return_parts.push("_ret_ptr".to_string());
                    }
                }
                _ => {
                    return_parts.push("_ret".to_string());
                }
            }
        }

        // Out/InOut params (skip InOut StructOpaque — data written back in-place)
        for (param, dir, mapped) in &all_mapped {
            if !is_scalar_output_returnable(*dir, mapped) {
                continue;
            }
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    return_parts.push(format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}) }}"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        {pname}_buf.truncate({pname}_len as usize);\n"));
                    out.push_str(&format!("        let {pname}_str = String::from_utf8_lossy(&{pname}_buf).into_owned();\n"));
                    return_parts.push(format!("{pname}_str"));
                }
                ConversionKind::EnumCast => {
                    let rt = &mapped.rust_type;
                    let actual_repr = param.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let {pname}_enum = {rt}::from_value({pname} as {actual_repr}).expect(\"unknown enum value\");\n"));
                    return_parts.push(format!("{pname}_enum"));
                }
                ConversionKind::StructOpaque => {
                    if is_struct_owned(param.struct_name.as_deref(), ctx) {
                        out.push_str(&format!("        let {pname}_owned = uika_runtime::OwnedStruct::from_bytes({pname}_buf);\n"));
                        return_parts.push(format!("{pname}_owned"));
                    } else {
                        out.push_str(&format!("        let {pname}_ptr = {pname}_buf.as_ptr();\n"));
                        out.push_str(&format!("        std::mem::forget({pname}_buf);\n"));
                        return_parts.push(format!("{pname}_ptr"));
                    }
                }
                ConversionKind::IntCast => {
                    let rt = &mapped.rust_type;
                    return_parts.push(format!("{pname} as {rt}"));
                }
                ConversionKind::FName => {
                    return_parts.push(pname.to_string());
                }
                _ => {
                    return_parts.push(pname.to_string());
                }
            }
        }

        match return_parts.len() {
            0 => {},
            1 => out.push_str(&format!("        {}\n", return_parts[0])),
            _ => out.push_str(&format!("        ({})\n", return_parts.join(", "))),
        }
    }

    out.push_str("    }\n\n");
}

// ---------------------------------------------------------------------------
// Container function implementation
// ---------------------------------------------------------------------------

/// Metadata about a container parameter tracked during code generation.
struct ContainerParamMeta<'a> {
    param: &'a ParamInfo,
    dir: ParamDirection,
    /// Index in the CPROPS array.
    index: usize,
}

/// Generate a function wrapper for functions that have container parameters.
/// Uses alloc_temp/free_temp for temp container lifecycle management.
fn generate_container_function(out: &mut String, entry: &FuncEntry, class_name: &str, ctx: &CodegenContext) {
    let func = &entry.func;
    let rust_fn_name = escape_reserved(&entry.rust_func_name);
    let func_id = entry.func_id;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);
    let ue_name = if func.ue_name.is_empty() { &entry.func_name } else { &func.ue_name };

    // Collect container params with their indices
    let mut container_params: Vec<ContainerParamMeta> = Vec::new();
    for param in &func.params {
        if is_container_param(param) {
            let dir = type_map::param_direction(param);
            let index = container_params.len();
            container_params.push(ContainerParamMeta { param, dir, index });
        }
    }
    let n_containers = container_params.len();

    // Classify all params and check support
    let mut return_param: Option<&ParamInfo> = None;
    let mut all_supported = true;

    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Return {
            return_param = Some(param);
        }
        if is_container_param(param) {
            if (dir == ParamDirection::In || dir == ParamDirection::InOut)
                && container_param_input_type(param, ctx).is_none()
            {
                all_supported = false;
                break;
            }
            if (dir == ParamDirection::Out || dir == ParamDirection::Return || dir == ParamDirection::InOut)
                && container_param_output_type(param, ctx).is_none()
            {
                all_supported = false;
                break;
            }
        } else {
            let mapped = type_map::map_property_type(
                &param.prop_type, param.class_name.as_deref(),
                param.struct_name.as_deref(), param.enum_name.as_deref(),
                param.enum_underlying_type.as_deref(),
                param.meta_class_name.as_deref(),
                param.interface_name.as_deref(),
            );
            if !mapped.supported {
                all_supported = false;
                break;
            }
        }
    }

    if !all_supported {
        out.push_str(&format!(
            "    // Skipped: {} (unsupported container inner type)\n\n",
            func.name
        ));
        return;
    }

    // Build return type
    let mut output_types = Vec::new();
    let mut scalar_return_mapped: Option<MappedType> = None;

    if let Some(rp) = return_param {
        if is_container_param(rp) {
            output_types.push(container_param_output_type(rp, ctx)
                    .expect("container return type should be resolvable"));
        } else {
            let rm = type_map::map_property_type(
                &rp.prop_type, rp.class_name.as_deref(),
                rp.struct_name.as_deref(), rp.enum_name.as_deref(),
                rp.enum_underlying_type.as_deref(),
                rp.meta_class_name.as_deref(),
                rp.interface_name.as_deref(),
            );
            output_types.push(scalar_out_rust_type_ctx(&rm, rp.struct_name.as_deref(), ctx));
            scalar_return_mapped = Some(rm);
        }
    }
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Out || dir == ParamDirection::InOut {
            if is_container_param(param) {
                output_types.push(container_param_output_type(param, ctx)
                        .expect("container out-param type should be resolvable"));
            } else {
                let rm = type_map::map_property_type(
                    &param.prop_type, param.class_name.as_deref(),
                    param.struct_name.as_deref(), param.enum_name.as_deref(),
                    param.enum_underlying_type.as_deref(),
                    param.meta_class_name.as_deref(),
                    param.interface_name.as_deref(),
                );
                if is_scalar_output_returnable(dir, &rm) {
                    output_types.push(scalar_out_rust_type_ctx(&rm, param.struct_name.as_deref(), ctx));
                }
            }
        }
    }
    let return_rust_type = build_return_type(&output_types);

    // === Emit Rust function signature ===
    let mut sig = String::new();
    if is_static {
        sig.push_str(&format!("    fn {rust_fn_name}("));
    } else {
        sig.push_str(&format!("    fn {rust_fn_name}(&self, "));
    }

    let mut default_unwraps: Vec<(String, String)> = Vec::new();
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Return || dir == ParamDirection::Out {
            continue;
        }
        let pname = escape_reserved(&to_snake_case(&param.name));
        if is_container_param(param) {
            let input_type = container_param_input_type(param, ctx)
                .expect("container input type should be resolvable");
            sig.push_str(&format!("{pname}: {input_type}, "));
        } else {
            let mapped = map_param(param);
            let has_default = dir == ParamDirection::In
                && defaults::parse_default_literal(param, &mapped, ctx).is_some();
            if has_default {
                let default_expr = defaults::parse_default_literal(param, &mapped, ctx)
                    .expect("default literal must be parseable (has_default was true)");
                default_unwraps.push((pname.clone(), default_expr));
            }
            match mapped.rust_to_ffi {
                ConversionKind::StringUtf8 => {
                    if has_default {
                        sig.push_str(&format!("{pname}: Option<&str>, "));
                    } else {
                        sig.push_str(&format!("{pname}: &str, "));
                    }
                }
                ConversionKind::StructOpaque if dir == ParamDirection::In
                    && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                {
                    let si = ctx.structs.get(param.struct_name.as_deref().expect("StructOpaque param must have struct_name"))
                            .expect("struct must exist in context");
                    sig.push_str(&format!(
                        "{pname}: &uika_runtime::OwnedStruct<{}>, ", si.cpp_name
                    ));
                }
                ConversionKind::StructOpaque if dir == ParamDirection::InOut => {
                    sig.push_str(&format!("{pname}: *mut u8, "));
                }
                _ => {
                    if has_default {
                        sig.push_str(&format!("{pname}: Option<{}>, ", mapped.rust_type));
                    } else {
                        sig.push_str(&format!("{pname}: {}, ", mapped.rust_type));
                    }
                }
            }
        }
    }
    if sig.ends_with(", ") {
        sig.truncate(sig.len() - 2);
    }
    if return_rust_type == "()" {
        sig.push(')');
    } else {
        sig.push_str(&format!(") -> {return_rust_type}"));
    }
    out.push_str(&sig);
    out.push_str(" {\n");

    // Unwrap defaulted params before any FFI conversion
    for (pname, default_expr) in &default_unwraps {
        out.push_str(&format!("        let {pname} = {pname}.unwrap_or({default_expr});\n"));
    }

    // === OnceLock for container FPropertyHandles ===
    let ue_name_len = ue_name.len();
    let ue_name_byte_lit = format!("b\"{}\\0\"", ue_name);

    out.push_str(&format!(
        "        const FN_ID: u32 = {func_id};\n\
         \x20       static CPROPS: std::sync::OnceLock<[uika_runtime::FPropertyHandle; {n_containers}]> = std::sync::OnceLock::new();\n\
         \x20       let __cprops = CPROPS.get_or_init(|| unsafe {{\n\
         \x20           let __ufunc = ((*uika_runtime::api().reflection).find_function_by_class)(\n\
         \x20               {class_name}::static_class(),\n\
         \x20               {ue_name_byte_lit}.as_ptr(), {ue_name_len});\n\
         \x20           [\n"
    ));
    for cp in &container_params {
        let param_name = &cp.param.name;
        let param_name_len = param_name.len();
        let param_byte_lit = format!("b\"{}\\0\"", param_name);
        out.push_str(&format!(
            "                ((*uika_runtime::api().reflection).get_function_param)(\n\
             \x20                   __ufunc, {param_byte_lit}.as_ptr(), {param_name_len}),\n"
        ));
    }
    out.push_str(
        "            ]\n\
         \x20       });\n"
    );

    // === FFI type signature ===
    let mut ffi_params = String::new();
    if !is_static {
        ffi_params.push_str("uika_runtime::UObjectHandle, ");
    }
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            ffi_params.push_str("*mut u8, *mut u8, "); // base, prop
        } else {
            let mapped = map_param(param);
            match dir {
                ParamDirection::In | ParamDirection::InOut => {
                    match mapped.rust_to_ffi {
                        ConversionKind::StringUtf8 => {
                            ffi_params.push_str("*const u8, u32, ");
                            if dir == ParamDirection::InOut {
                                ffi_params.push_str("*mut u8, u32, *mut u32, ");
                            }
                        }
                        ConversionKind::ObjectRef => ffi_params.push_str("uika_runtime::UObjectHandle, "),
                        ConversionKind::EnumCast => ffi_params.push_str(&format!("{}, ", mapped.rust_ffi_type)),
                        ConversionKind::StructOpaque => {
                            if dir == ParamDirection::InOut {
                                ffi_params.push_str("*mut u8, ");
                            } else {
                                ffi_params.push_str("*const u8, ");
                            }
                        }
                        _ => ffi_params.push_str(&format!("{}, ", mapped.rust_ffi_type)),
                    }
                }
                ParamDirection::Out | ParamDirection::Return => {
                    match mapped.ffi_to_rust {
                        ConversionKind::StringUtf8 => ffi_params.push_str("*mut u8, u32, *mut u32, "),
                        ConversionKind::ObjectRef => ffi_params.push_str("*mut uika_runtime::UObjectHandle, "),
                        ConversionKind::StructOpaque => ffi_params.push_str("*mut u8, "),
                        ConversionKind::EnumCast => ffi_params.push_str(&format!("*mut {}, ", mapped.rust_ffi_type)),
                        _ => ffi_params.push_str(&format!("*mut {}, ", mapped.rust_ffi_type)),
                    }
                }
            }
        }
    }
    if ffi_params.ends_with(", ") {
        ffi_params.truncate(ffi_params.len() - 2);
    }

    out.push_str(&format!(
        "        type Fn = unsafe extern \"C\" fn({ffi_params}) -> uika_runtime::UikaErrorCode;\n\
         \x20       let __uika_fn: Fn = unsafe {{ std::mem::transmute(*(uika_runtime::api().func_table.add(FN_ID as usize))) }};\n"
    ));

    // === Get handle (pre-validated via ValidHandle) ===
    if !is_static {
        out.push_str("        let h = self.handle();\n");
    }

    // === Alloc temps for all container params ===
    for cp in &container_params {
        let idx = cp.index;
        out.push_str(&format!(
            "        let __temp_{idx} = unsafe {{ ((*uika_runtime::api().container).alloc_temp)(__cprops[{idx}]) }};\n"
        ));
    }

    // === Populate input containers ===
    for cp in &container_params {
        if cp.dir != ParamDirection::In && cp.dir != ParamDirection::InOut {
            continue;
        }
        let idx = cp.index;
        let pname = escape_reserved(&to_snake_case(&cp.param.name));
        emit_container_populate(out, cp.param, idx, &pname, ctx);
    }

    // === Declare scalar output variables ===
    let ret_mapped = scalar_return_mapped.as_ref();
    if let Some(rm) = ret_mapped {
        match rm.ffi_to_rust {
            ConversionKind::ObjectRef => {
                out.push_str("        let mut __scalar_ret = uika_runtime::UObjectHandle(std::ptr::null_mut());\n");
            }
            ConversionKind::StringUtf8 => {
                out.push_str("        let mut __scalar_ret_buf = vec![0u8; 512];\n");
                out.push_str("        let mut __scalar_ret_len: u32 = 0;\n");
            }
            ConversionKind::EnumCast => {
                out.push_str(&format!("        let mut __scalar_ret: {} = 0;\n", rm.rust_ffi_type));
            }
            ConversionKind::StructOpaque => {
                out.push_str("        let mut __scalar_ret_buf = vec![0u8; 256];\n");
            }
            _ => {
                let default = properties::default_value_for(&rm.rust_ffi_type);
                out.push_str(&format!("        let mut __scalar_ret = {default};\n"));
            }
        }
    }

    // Scalar Out params (non-container)
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Out && !is_container_param(param) {
            let mapped = map_param(param);
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::StructOpaque => {
                    out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 256];\n"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
                    out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
                }
                ConversionKind::ObjectRef => {
                    out.push_str(&format!("        let mut {pname} = uika_runtime::UObjectHandle(std::ptr::null_mut());\n"));
                }
                ConversionKind::EnumCast => {
                    out.push_str(&format!("        let mut {pname}: {} = 0;\n", mapped.rust_ffi_type));
                }
                _ => {
                    let default = properties::default_value_for(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let mut {pname} = {default};\n"));
                }
            }
        }
        // InOut string/text params need output buffers for the modified value
        if dir == ParamDirection::InOut && !is_container_param(param) {
            let mapped = map_param(param);
            if mapped.ffi_to_rust == ConversionKind::StringUtf8 {
                let pname = escape_reserved(&to_snake_case(&param.name));
                out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
                out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
            }
        }
    }

    // === FFI call (deferred error check) ===
    out.push_str("        let __result = unsafe { __uika_fn(");
    if !is_static {
        out.push_str("h, ");
    }
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            let cp = container_params.iter().find(|c| std::ptr::eq(c.param, param))
                .expect("container param must have matching metadata");
            let idx = cp.index;
            out.push_str(&format!("__temp_{idx}, __cprops[{idx}].0 as *mut u8, "));
        } else {
            let pname = escape_reserved(&to_snake_case(&param.name));
            let mapped = map_param(param);
            match dir {
                ParamDirection::In | ParamDirection::InOut => {
                    match mapped.rust_to_ffi {
                        ConversionKind::StringUtf8 => {
                            out.push_str(&format!("{pname}.as_ptr(), {pname}.len() as u32, "));
                            if dir == ParamDirection::InOut {
                                out.push_str(&format!("{pname}_buf.as_mut_ptr(), {pname}_buf.len() as u32, &mut {pname}_len, "));
                            }
                        }
                        ConversionKind::ObjectRef => {
                            out.push_str(&format!("{pname}.raw(), "));
                        }
                        ConversionKind::EnumCast => {
                            out.push_str(&format!("{pname} as {}, ", mapped.rust_ffi_type));
                        }
                        ConversionKind::StructOpaque if dir == ParamDirection::In
                            && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                        {
                            out.push_str(&format!("{pname}.as_bytes().as_ptr(), "));
                        }
                        _ => {
                            out.push_str(&format!("{pname}, "));
                        }
                    }
                }
                ParamDirection::Out => {
                    match mapped.ffi_to_rust {
                        ConversionKind::StructOpaque => {
                            out.push_str(&format!("{pname}_buf.as_mut_ptr(), "));
                        }
                        ConversionKind::StringUtf8 => {
                            out.push_str(&format!("{pname}_buf.as_mut_ptr(), {pname}_buf.len() as u32, &mut {pname}_len, "));
                        }
                        _ => {
                            out.push_str(&format!("&mut {pname}, "));
                        }
                    }
                }
                ParamDirection::Return => {
                    let rm = ret_mapped.expect("return param must have mapped type");
                    match rm.ffi_to_rust {
                        ConversionKind::StringUtf8 => {
                            out.push_str("__scalar_ret_buf.as_mut_ptr(), __scalar_ret_buf.len() as u32, &mut __scalar_ret_len, ");
                        }
                        ConversionKind::ObjectRef => {
                            out.push_str("&mut __scalar_ret, ");
                        }
                        ConversionKind::StructOpaque => {
                            out.push_str("__scalar_ret_buf.as_mut_ptr(), ");
                        }
                        _ => {
                            out.push_str("&mut __scalar_ret, ");
                        }
                    }
                }
            }
        }
    }
    // Remove trailing comma+space
    let out_len = out.len();
    if out.ends_with(", ") {
        out.truncate(out_len - 2);
    }
    out.push_str(") };\n");

    // === Read output containers (only on success) ===
    for cp in &container_params {
        if cp.dir != ParamDirection::Out && cp.dir != ParamDirection::Return && cp.dir != ParamDirection::InOut {
            continue;
        }
        let idx = cp.index;
        emit_container_read(out, cp.param, idx, ctx);
    }

    // === Free ALL temps ===
    out.push_str("        unsafe {\n");
    for cp in &container_params {
        let idx = cp.index;
        out.push_str(&format!(
            "            ((*uika_runtime::api().container).free_temp)(__cprops[{idx}], __temp_{idx});\n"
        ));
    }
    out.push_str("        }\n");

    // === Assert success (infallible after pre-validation) ===
    out.push_str("        uika_runtime::ffi_infallible(__result);\n");

    // === Return ===
    emit_container_return(out, return_param, ret_mapped, &container_params, &func.params, ctx);

    out.push_str("    }\n\n");
}

/// Emit code to populate an input container from a Rust slice.
fn emit_container_populate(out: &mut String, param: &ParamInfo, idx: usize, pname: &str, ctx: &CodegenContext) {
    let elem_type = container_elem_type_str(param, ctx)
        .expect("container element type must be resolvable");
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __arr = uika_runtime::UeArray::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           for __elem in {pname} {{\n\
                 \x20               let _ = __arr.push(__elem);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        "SetProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __set = uika_runtime::UeSet::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           for __elem in {pname} {{\n\
                 \x20               let _ = __set.add(__elem);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        "MapProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __map = uika_runtime::UeMap::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           for (__k, __v) in {pname} {{\n\
                 \x20               let _ = __map.add(__k, __v);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        _ => {}
    }
}

/// Emit code to read an output container into a Vec.
fn emit_container_read(out: &mut String, param: &ParamInfo, idx: usize, ctx: &CodegenContext) {
    let elem_type = container_elem_type_str(param, ctx)
        .expect("container element type must be resolvable");
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            out.push_str(&format!(
                "        let __out_{idx} = if __result == uika_runtime::UikaErrorCode::Ok {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __arr = uika_runtime::UeArray::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           let __len = __arr.len().unwrap_or(0);\n\
                 \x20           let mut __v = Vec::with_capacity(__len);\n\
                 \x20           for __i in 0..__len {{\n\
                 \x20               if let Ok(__val) = __arr.get(__i) {{ __v.push(__val); }}\n\
                 \x20           }}\n\
                 \x20           __v\n\
                 \x20       }} else {{ Vec::new() }};\n"
            ));
        }
        "SetProperty" => {
            out.push_str(&format!(
                "        let __out_{idx} = if __result == uika_runtime::UikaErrorCode::Ok {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __set = uika_runtime::UeSet::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           let __len = __set.len().unwrap_or(0);\n\
                 \x20           let mut __v = Vec::with_capacity(__len);\n\
                 \x20           for __i in 0..__len {{\n\
                 \x20               if let Ok(__val) = __set.get_element(__i) {{ __v.push(__val); }}\n\
                 \x20           }}\n\
                 \x20           __v\n\
                 \x20       }} else {{ Vec::new() }};\n"
            ));
        }
        "MapProperty" => {
            out.push_str(&format!(
                "        let __out_{idx} = if __result == uika_runtime::UikaErrorCode::Ok {{\n\
                 \x20           let __h = uika_runtime::UObjectHandle(__temp_{idx} as *mut std::ffi::c_void);\n\
                 \x20           let __map = uika_runtime::UeMap::<{elem_type}>::new(__h, __cprops[{idx}]);\n\
                 \x20           let __len = __map.len().unwrap_or(0);\n\
                 \x20           let mut __v = Vec::with_capacity(__len);\n\
                 \x20           for __i in 0..__len {{\n\
                 \x20               if let Ok(__pair) = __map.get_pair(__i) {{ __v.push(__pair); }}\n\
                 \x20           }}\n\
                 \x20           __v\n\
                 \x20       }} else {{ Vec::new() }};\n"
            ));
        }
        _ => {}
    }
}

/// Emit the final return expression, assembling scalar returns and container outputs (infallible).
fn emit_container_return(
    out: &mut String,
    return_param: Option<&ParamInfo>,
    ret_mapped: Option<&MappedType>,
    container_params: &[ContainerParamMeta],
    func_params: &[ParamInfo],
    ctx: &CodegenContext,
) {
    let mut return_parts = Vec::new();

    // Scalar or container return value
    if let Some(rp) = return_param {
        if is_container_param(rp) {
            let cp = container_params.iter().find(|c| c.dir == ParamDirection::Return)
                .expect("container return param must exist");
            return_parts.push(format!("__out_{}", cp.index));
        } else if let Some(rm) = ret_mapped {
            match rm.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    return_parts.push("unsafe { uika_runtime::UObjectRef::from_raw(__scalar_ret) }".to_string());
                }
                ConversionKind::StringUtf8 => {
                    out.push_str("        __scalar_ret_buf.truncate(__scalar_ret_len as usize);\n");
                    out.push_str("        let __scalar_str = String::from_utf8_lossy(&__scalar_ret_buf).into_owned();\n");
                    return_parts.push("__scalar_str".to_string());
                }
                ConversionKind::EnumCast => {
                    let rt = &rm.rust_type;
                    let rp_ref = return_param.expect("return_param must be Some in return conversion");
                    let actual_repr = rp_ref.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&rm.rust_ffi_type);
                    out.push_str(&format!(
                        "        let __scalar_enum = {rt}::from_value(__scalar_ret as {actual_repr}).expect(\"unknown enum value\");\n"
                    ));
                    return_parts.push("__scalar_enum".to_string());
                }
                ConversionKind::StructOpaque => {
                    let rp_ref = return_param.expect("return_param must be Some in return conversion");
                    if is_struct_owned(rp_ref.struct_name.as_deref(), ctx) {
                        out.push_str("        let __scalar_owned = uika_runtime::OwnedStruct::from_bytes(__scalar_ret_buf);\n");
                        return_parts.push("__scalar_owned".to_string());
                    } else {
                        out.push_str("        let __scalar_ptr = __scalar_ret_buf.as_ptr();\n");
                        out.push_str("        std::mem::forget(__scalar_ret_buf);\n");
                        return_parts.push("__scalar_ptr".to_string());
                    }
                }
                _ => {
                    return_parts.push("__scalar_ret".to_string());
                }
            }
        }
    }

    // Out/InOut params in original parameter order (must match return type construction)
    for param in func_params {
        let dir = type_map::param_direction(param);
        if dir != ParamDirection::Out && dir != ParamDirection::InOut {
            continue;
        }

        if is_container_param(param) {
            let cp = container_params.iter().find(|c| std::ptr::eq(c.param, param))
                .expect("container param must have matching metadata");
            return_parts.push(format!("__out_{}", cp.index));
        } else {
            let mapped = map_param(param);
            if !is_scalar_output_returnable(dir, &mapped) {
                continue;
            }
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    return_parts.push(format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}) }}"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        {pname}_buf.truncate({pname}_len as usize);\n"));
                    out.push_str(&format!("        let {pname}_str = String::from_utf8_lossy(&{pname}_buf).into_owned();\n"));
                    return_parts.push(format!("{pname}_str"));
                }
                ConversionKind::EnumCast => {
                    let rt = &mapped.rust_type;
                    let actual_repr = param.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let {pname}_enum = {rt}::from_value({pname} as {actual_repr}).expect(\"unknown enum value\");\n"));
                    return_parts.push(format!("{pname}_enum"));
                }
                ConversionKind::StructOpaque => {
                    if is_struct_owned(param.struct_name.as_deref(), ctx) {
                        out.push_str(&format!("        let {pname}_owned = uika_runtime::OwnedStruct::from_bytes({pname}_buf);\n"));
                        return_parts.push(format!("{pname}_owned"));
                    } else {
                        out.push_str(&format!("        let {pname}_ptr = {pname}_buf.as_ptr();\n"));
                        out.push_str(&format!("        std::mem::forget({pname}_buf);\n"));
                        return_parts.push(format!("{pname}_ptr"));
                    }
                }
                ConversionKind::IntCast => {
                    let rt = &mapped.rust_type;
                    return_parts.push(format!("{pname} as {rt}"));
                }
                ConversionKind::FName => {
                    return_parts.push(pname.to_string());
                }
                _ => {
                    return_parts.push(pname.to_string());
                }
            }
        }
    }

    match return_parts.len() {
        0 => {},
        1 => out.push_str(&format!("        {}\n", return_parts[0])),
        _ => out.push_str(&format!("        ({})\n", return_parts.join(", "))),
    }
}

/// Map a ParamInfo to its MappedType (convenience helper).
fn map_param(param: &ParamInfo) -> MappedType {
    type_map::map_property_type(
        &param.prop_type,
        param.class_name.as_deref(),
        param.struct_name.as_deref(),
        param.enum_name.as_deref(),
        param.enum_underlying_type.as_deref(),
        param.meta_class_name.as_deref(),
        param.interface_name.as_deref(),
    )
}
