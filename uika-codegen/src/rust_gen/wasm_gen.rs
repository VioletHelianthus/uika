// WASM bridge code generation for codegen-produced functions (~3,700).
//
// Three outputs:
// 1. wasm_fn_imports.rs — extern import declarations for the WASM guest
// 2. cfg(wasm32) function bodies — inline in each generated binding
// 3. wasm_host_codegen_funcs.rs — host function registration (Phase 3.1b)
//
// This module handles all three outputs.

use crate::context::{CodegenContext, FuncEntry};
use crate::naming::{escape_reserved, to_snake_case};
use crate::schema::*;
use crate::type_map::{self, ConversionKind, MappedType, ParamDirection};

use super::classes::{is_container_param, map_param, is_struct_owned};

// ---------------------------------------------------------------------------
// WASM type mapping
// ---------------------------------------------------------------------------

/// WASM type for a given FFI parameter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValType {
    I32,
    I64,
    F32,
    F64,
}

impl std::fmt::Display for WasmValType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmValType::I32 => write!(f, "i32"),
            WasmValType::I64 => write!(f, "i64"),
            WasmValType::F32 => write!(f, "f32"),
            WasmValType::F64 => write!(f, "f64"),
        }
    }
}

/// Map a Rust FFI type to its WASM value type.
fn ffi_type_to_wasm(ffi_type: &str) -> WasmValType {
    match ffi_type {
        "bool" | "u8" | "i8" | "i16" | "u16" | "i32" | "u32" => WasmValType::I32,
        "i64" | "u64" => WasmValType::I64,
        "f32" => WasmValType::F32,
        "f64" => WasmValType::F64,
        // Handle types are i64 in WASM
        t if t.contains("UObjectHandle")
            || t.contains("UClassHandle")
            || t.contains("FPropertyHandle")
            || t.contains("UFunctionHandle")
            || t.contains("UStructHandle")
            || t.contains("FNameHandle") => WasmValType::I64,
        // UikaErrorCode → i32
        t if t.contains("UikaErrorCode") => WasmValType::I32,
        _ => WasmValType::I32,
    }
}

/// Classify a parameter into its WASM calling convention.
/// Returns the list of (wasm_type, role) pairs for the parameter.
#[derive(Debug, Clone)]
pub enum WasmParamSlot {
    /// A single WASM value (handle, scalar, enum)
    Single(WasmValType),
    /// A ptr+len pair for string/struct buffer (two i32 values)
    PtrLen,
    /// A ptr+len+out_len triple for string output buffer (three i32 values)
    PtrLenOutLen,
    /// An out pointer to a scalar (one i32 — WASM memory offset)
    ScalarOut(WasmValType),
    /// Native pointer pass-through (i64 — opaque host pointer)
    NativePtr,
    /// Container param: base (i64) + prop (i64) — both native pointers
    ContainerPtrs,
}

/// Determine WASM calling convention slots for one parameter.
fn param_wasm_slots(param: &ParamInfo, dir: ParamDirection, mapped: &MappedType) -> Vec<WasmParamSlot> {
    if is_container_param(param) {
        return vec![WasmParamSlot::ContainerPtrs];
    }

    match dir {
        ParamDirection::In => {
            match mapped.rust_to_ffi {
                ConversionKind::StringUtf8 => vec![WasmParamSlot::PtrLen],
                ConversionKind::StructOpaque => vec![WasmParamSlot::PtrLen],
                ConversionKind::ObjectRef => vec![WasmParamSlot::Single(WasmValType::I64)],
                ConversionKind::FName => vec![WasmParamSlot::Single(WasmValType::I64)],
                ConversionKind::EnumCast => vec![WasmParamSlot::Single(WasmValType::I32)],
                _ => vec![WasmParamSlot::Single(ffi_type_to_wasm(&mapped.rust_ffi_type))],
            }
        }
        ParamDirection::InOut => {
            match mapped.rust_to_ffi {
                ConversionKind::StringUtf8 => {
                    // Input ptr+len, then output buf+len+out_len
                    vec![WasmParamSlot::PtrLen, WasmParamSlot::PtrLenOutLen]
                }
                ConversionKind::StructOpaque => {
                    // Mutable struct pointer — treated as ptr+len for WASM
                    vec![WasmParamSlot::PtrLen]
                }
                ConversionKind::ObjectRef => vec![WasmParamSlot::Single(WasmValType::I64)],
                _ => vec![WasmParamSlot::Single(ffi_type_to_wasm(&mapped.rust_ffi_type))],
            }
        }
        ParamDirection::Out => {
            match mapped.ffi_to_rust {
                ConversionKind::StringUtf8 => vec![WasmParamSlot::PtrLenOutLen],
                ConversionKind::StructOpaque => vec![WasmParamSlot::PtrLen],
                ConversionKind::ObjectRef => vec![WasmParamSlot::ScalarOut(WasmValType::I64)],
                ConversionKind::EnumCast => vec![WasmParamSlot::ScalarOut(WasmValType::I32)],
                _ => vec![WasmParamSlot::ScalarOut(ffi_type_to_wasm(&mapped.rust_ffi_type))],
            }
        }
        ParamDirection::Return => {
            match mapped.ffi_to_rust {
                ConversionKind::StringUtf8 => vec![WasmParamSlot::PtrLenOutLen],
                ConversionKind::StructOpaque => vec![WasmParamSlot::PtrLen],
                ConversionKind::ObjectRef => vec![WasmParamSlot::ScalarOut(WasmValType::I64)],
                ConversionKind::EnumCast => vec![WasmParamSlot::ScalarOut(WasmValType::I32)],
                _ => vec![WasmParamSlot::ScalarOut(ffi_type_to_wasm(&mapped.rust_ffi_type))],
            }
        }
    }
}

/// Check if a function exceeds wasmtime's func_wrap param limit (16 including Caller).
/// Returns true if the function can be registered as a WASM host function.
pub fn wasm_func_within_param_limit(entry: &FuncEntry, ctx: &CodegenContext) -> bool {
    let func = &entry.func;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);
    let mut count: usize = 1; // Caller<HostState>
    if !is_static {
        count += 1; // obj: i64
    }
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            count += 2; // base + prop
        } else {
            let mapped = map_param(param);
            if !mapped.supported {
                return false;
            }
            for slot in &param_wasm_slots(param, dir, &mapped) {
                match slot {
                    WasmParamSlot::Single(_) | WasmParamSlot::ScalarOut(_) | WasmParamSlot::NativePtr => count += 1,
                    WasmParamSlot::PtrLen => count += 2,
                    WasmParamSlot::PtrLenOutLen => count += 3,
                    WasmParamSlot::ContainerPtrs => count += 2,
                }
            }
        }
    }
    count <= 16
}

// ---------------------------------------------------------------------------
// Guest extern import generation (wasm_fn_imports.rs)
// ---------------------------------------------------------------------------

/// Generate the complete wasm_fn_imports.rs file content.
pub fn generate_wasm_fn_imports(func_table: &[FuncEntry], ctx: &CodegenContext) -> String {
    let mut out = String::with_capacity(128 * 1024);
    out.push_str("// Auto-generated WASM extern imports for codegen functions.\n");
    out.push_str("// Do not edit manually.\n\n");
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("#[link(wasm_import_module = \"uika_fn\")]\n");
    out.push_str("unsafe extern \"C\" {\n");

    for entry in func_table {
        if let Some(sig) = build_extern_import_sig(entry, ctx) {
            out.push_str(&format!(
                "    // {}.{}\n",
                entry.class_name, entry.func_name
            ));
            out.push_str(&format!("    pub fn {}({}) -> i32;\n\n", wasm_fn_name(entry.func_id), sig));
        }
    }

    out.push_str("}\n");
    out
}

/// Build the extern import function name: uika_fn_{func_id}
fn wasm_fn_name(func_id: u32) -> String {
    format!("uika_fn_{func_id}")
}

/// Build the extern import parameter signature string.
/// Returns None if the function has unsupported params.
fn build_extern_import_sig(entry: &FuncEntry, ctx: &CodegenContext) -> Option<String> {
    let func = &entry.func;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);

    let mut params = Vec::new();

    // Instance method: first param is the object handle
    if !is_static {
        params.push("obj: i64".to_string());
    }

    for param in &func.params {
        let dir = type_map::param_direction(param);

        if is_container_param(param) {
            // Verify container is supported
            if dir == ParamDirection::In || dir == ParamDirection::InOut {
                if super::classes::container_param_input_type(param, ctx).is_none() {
                    return None;
                }
            }
            if dir == ParamDirection::Out || dir == ParamDirection::Return || dir == ParamDirection::InOut {
                if super::classes::container_param_output_type(param, ctx).is_none() {
                    return None;
                }
            }
            // Container params: base (i64, native ptr) + prop (i64, native ptr)
            let pname = to_snake_case(&param.name);
            params.push(format!("{pname}_base: i64"));
            params.push(format!("{pname}_prop: i64"));
            continue;
        }

        let mapped = map_param(param);
        if !mapped.supported {
            return None;
        }

        let pname = escape_reserved(&to_snake_case(&param.name));
        let slots = param_wasm_slots(param, dir, &mapped);

        for slot in &slots {
            match slot {
                WasmParamSlot::Single(wt) => {
                    params.push(format!("{pname}: {wt}"));
                }
                WasmParamSlot::PtrLen => {
                    params.push(format!("{pname}_ptr: i32"));
                    params.push(format!("{pname}_len: i32"));
                }
                WasmParamSlot::PtrLenOutLen => {
                    params.push(format!("{pname}_buf: i32"));
                    params.push(format!("{pname}_buf_len: i32"));
                    params.push(format!("{pname}_out_len: i32"));
                }
                WasmParamSlot::ScalarOut(_wt) => {
                    params.push(format!("{pname}_out: i32"));
                }
                WasmParamSlot::NativePtr => {
                    params.push(format!("{pname}: i64"));
                }
                WasmParamSlot::ContainerPtrs => {
                    // Already handled above
                    unreachable!();
                }
            }
        }
    }

    // Must match the host-side limit: func_wrap allows at most 16 closure params
    // (Caller + 15 actual params). Skip functions that exceed this.
    if params.len() > 15 {
        return None;
    }

    Some(params.join(", "))
}

// ---------------------------------------------------------------------------
// Guest function body generation (cfg(wasm32) branch)
// ---------------------------------------------------------------------------

/// Generate the `#[cfg(target_arch = "wasm32")]` function body for a scalar function.
/// This is emitted inside the function, after the signature and opening brace.
pub fn generate_wasm_scalar_body(
    out: &mut String,
    entry: &FuncEntry,
    ctx: &CodegenContext,
) {
    let func = &entry.func;
    let func_id = entry.func_id;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);

    // Classify params
    let mut return_param: Option<&ParamInfo> = None;
    let mut all_mapped: Vec<(&ParamInfo, ParamDirection, MappedType)> = Vec::new();

    for param in &func.params {
        let dir = type_map::param_direction(param);
        let mapped = map_param(param);
        if dir == ParamDirection::Return {
            return_param = Some(param);
        }
        all_mapped.push((param, dir, mapped));
    }

    let ret_mapped = return_param.map(|rp| map_param(rp));

    // Get handle for instance methods
    if !is_static {
        out.push_str("        let h = self.handle();\n");
    }

    // Declare output variables (same as native, but handles use wasm-compatible defaults)
    if let Some(_rp) = return_param {
        let rm = ret_mapped.as_ref().unwrap();
        match rm.ffi_to_rust {
            ConversionKind::ObjectRef => {
                out.push_str("        let mut _ret: u64 = 0;\n");
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
                let default = super::properties::default_value_for(&rm.rust_ffi_type);
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
                    out.push_str(&format!("        let mut {pname}: u64 = 0;\n"));
                }
                ConversionKind::EnumCast => {
                    out.push_str(&format!("        let mut {pname}: {} = 0;\n", mapped.rust_ffi_type));
                }
                _ => {
                    let default = super::properties::default_value_for(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let mut {pname} = {default};\n"));
                }
            }
        }
        if *dir == ParamDirection::InOut && mapped.ffi_to_rust == ConversionKind::StringUtf8 {
            let pname = escape_reserved(&to_snake_case(&param.name));
            out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
            out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
        }
    }

    // Pre-declare temp buffers for owned struct In params (need to_bytes() before the call).
    for (param, dir, mapped) in &all_mapped {
        if *dir == ParamDirection::In
            && mapped.rust_to_ffi == ConversionKind::StructOpaque
            && is_struct_owned(param.struct_name.as_deref(), ctx)
        {
            let pname = escape_reserved(&to_snake_case(&param.name));
            out.push_str(&format!("        let {pname}_bytes = {pname}.to_bytes();\n"));
        }
    }

    // Build the FFI call via extern import
    out.push_str(&format!(
        "        let __wasm_err = unsafe {{ crate::wasm_fn_imports::{}(",
        wasm_fn_name(func_id)
    ));

    // Arguments
    if !is_static {
        out.push_str("h.0 as i64, ");
    }
    for (param, dir, mapped) in &all_mapped {
        let pname = escape_reserved(&to_snake_case(&param.name));
        match dir {
            ParamDirection::In | ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!("{pname}.as_ptr() as i32, {pname}.len() as i32, "));
                        if *dir == ParamDirection::InOut {
                            out.push_str(&format!(
                                "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, &mut {pname}_len as *mut u32 as i32, "
                            ));
                        }
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!("{pname}.raw_handle().0 as i64, "));
                    }
                    ConversionKind::EnumCast => {
                        out.push_str(&format!("{pname} as {} as i32, ", mapped.rust_ffi_type));
                    }
                    ConversionKind::FName => {
                        out.push_str(&format!("{pname}.0 as i64, "));
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::In
                        && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                    {
                        // Owned struct: use pre-declared bytes variable.
                        out.push_str(&format!(
                            "{pname}_bytes.as_ptr() as i32, {pname}_bytes.len() as i32, "
                        ));
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::In => {
                        // Non-owned struct: *const u8 pointer in WASM memory.
                        // Pass ptr and 0 for size; host determines actual size from reflection.
                        out.push_str(&format!("{pname} as i32, 0i32, "));
                    }
                    ConversionKind::StructOpaque if *dir == ParamDirection::InOut => {
                        // Mutable struct pointer passed as ptr+len
                        out.push_str(&format!("{pname} as i32, 256i32, "));
                    }
                    ConversionKind::Identity if mapped.rust_ffi_type == "bool" => {
                        out.push_str(&format!("{pname} as i32, "));
                    }
                    _ => {
                        let wt = ffi_type_to_wasm(&mapped.rust_ffi_type);
                        out.push_str(&format!("{pname} as {wt}, "));
                    }
                }
            }
            ParamDirection::Out => {
                match mapped.ffi_to_rust {
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!(
                            "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, "
                        ));
                    }
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, &mut {pname}_len as *mut u32 as i32, "
                        ));
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!("&mut {pname} as *mut u64 as i32, "));
                    }
                    _ => {
                        out.push_str(&format!(
                            "&mut {pname} as *mut {} as i32, ",
                            mapped.rust_ffi_type
                        ));
                    }
                }
            }
            ParamDirection::Return => {
                let rm = ret_mapped.as_ref().unwrap();
                match rm.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str(
                            "_ret_buf.as_mut_ptr() as i32, _ret_buf.len() as i32, &mut _ret_len as *mut u32 as i32, "
                        );
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str("&mut _ret as *mut u64 as i32, ");
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(
                            "_ret_struct_buf.as_mut_ptr() as i32, _ret_struct_buf.len() as i32, "
                        );
                    }
                    _ => {
                        out.push_str(&format!(
                            "&mut _ret as *mut {} as i32, ",
                            rm.rust_ffi_type
                        ));
                    }
                }
            }
        }
    }
    // Remove trailing comma+space
    if out.ends_with(", ") {
        out.truncate(out.len() - 2);
    }
    out.push_str(") };\n");

    // Error check
    out.push_str("        uika_runtime::ffi_infallible(unsafe { core::mem::transmute(__wasm_err as u32) });\n");

    // Return conversion (same logic as native, but ObjectRef handles use u64)
    {
        let mut return_parts = Vec::new();

        if return_param.is_some() {
            let rm = ret_mapped.as_ref().unwrap();
            match rm.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    out.push_str("        let _ret_handle = uika_runtime::UObjectHandle(_ret);\n");
                    return_parts.push("unsafe { uika_runtime::UObjectRef::from_raw(_ret_handle) }".to_string());
                }
                ConversionKind::StringUtf8 => {
                    out.push_str("        _ret_buf.truncate(_ret_len as usize);\n");
                    out.push_str("        let _ret_str = String::from_utf8_lossy(&_ret_buf).into_owned();\n");
                    return_parts.push("_ret_str".to_string());
                }
                ConversionKind::EnumCast => {
                    let rt = &rm.rust_type;
                    let rp = return_param.unwrap();
                    let actual_repr = rp.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&rm.rust_ffi_type);
                    out.push_str(&format!(
                        "        let _ret_enum = {rt}::from_value(_ret as {actual_repr}).expect(\"unknown enum value\");\n"
                    ));
                    return_parts.push("_ret_enum".to_string());
                }
                ConversionKind::StructOpaque => {
                    let rp = return_param.unwrap();
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

        for (param, dir, mapped) in &all_mapped {
            if !super::classes::is_scalar_output_returnable(*dir, mapped) {
                continue;
            }
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    out.push_str(&format!(
                        "        let {pname}_handle = uika_runtime::UObjectHandle({pname});\n"
                    ));
                    return_parts.push(format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}_handle) }}"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        {pname}_buf.truncate({pname}_len as usize);\n"));
                    out.push_str(&format!(
                        "        let {pname}_str = String::from_utf8_lossy(&{pname}_buf).into_owned();\n"
                    ));
                    return_parts.push(format!("{pname}_str"));
                }
                ConversionKind::EnumCast => {
                    let rt = &mapped.rust_type;
                    let actual_repr = param.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&mapped.rust_ffi_type);
                    out.push_str(&format!(
                        "        let {pname}_enum = {rt}::from_value({pname} as {actual_repr}).expect(\"unknown enum value\");\n"
                    ));
                    return_parts.push(format!("{pname}_enum"));
                }
                ConversionKind::StructOpaque => {
                    if is_struct_owned(param.struct_name.as_deref(), ctx) {
                        out.push_str(&format!(
                            "        let {pname}_owned = uika_runtime::OwnedStruct::from_bytes({pname}_buf);\n"
                        ));
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
                    // {pname} is already FNameHandle (declared via Default::default())
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
}

/// Generate the `#[cfg(target_arch = "wasm32")]` function body for a container function.
pub fn generate_wasm_container_body(
    out: &mut String,
    entry: &FuncEntry,
    class_name: &str,
    ctx: &CodegenContext,
) {
    let func = &entry.func;
    let func_id = entry.func_id;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);
    let ue_name = if func.ue_name.is_empty() { &entry.func_name } else { &func.ue_name };

    // Collect container params
    let mut container_indices: Vec<(usize, &ParamInfo, ParamDirection)> = Vec::new();
    for param in &func.params {
        if is_container_param(param) {
            let dir = type_map::param_direction(param);
            let idx = container_indices.len();
            container_indices.push((idx, param, dir));
        }
    }
    let n_containers = container_indices.len();

    // Classify params
    let mut return_param: Option<&ParamInfo> = None;
    let mut scalar_return_mapped: Option<MappedType> = None;

    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::Return {
            return_param = Some(param);
            if !is_container_param(param) {
                scalar_return_mapped = Some(map_param(param));
            }
        }
    }
    let ret_mapped = scalar_return_mapped.as_ref();

    // OnceLock for container FPropertyHandles (same as native — uses ffi_dispatch)
    let ue_name_len = ue_name.len();
    let ue_name_byte_lit = format!("b\"{}\\0\"", ue_name);

    out.push_str(&format!(
        "        const FN_ID: u32 = {func_id};\n\
         \x20       static CPROPS: std::sync::OnceLock<[uika_runtime::FPropertyHandle; {n_containers}]> = std::sync::OnceLock::new();\n\
         \x20       let __cprops = CPROPS.get_or_init(|| unsafe {{\n\
         \x20           let __ufunc = uika_runtime::ffi_dispatch::reflection_find_function_by_class(\n\
         \x20               {class_name}::static_class(),\n\
         \x20               {ue_name_byte_lit}.as_ptr(), {ue_name_len});\n\
         \x20           [\n"
    ));
    for (_, cp_param, _) in &container_indices {
        let param_name = &cp_param.name;
        let param_name_len = param_name.len();
        let param_byte_lit = format!("b\"{}\\0\"", param_name);
        out.push_str(&format!(
            "                uika_runtime::ffi_dispatch::reflection_get_function_param(\n\
             \x20                   __ufunc, {param_byte_lit}.as_ptr(), {param_name_len}),\n"
        ));
    }
    out.push_str(
        "            ]\n\
         \x20       });\n"
    );

    // Get handle
    if !is_static {
        out.push_str("        let h = self.handle();\n");
    }

    // Alloc temps (uses ffi_dispatch)
    for (idx, _, _) in &container_indices {
        out.push_str(&format!(
            "        let __temp_{idx} = unsafe {{ uika_runtime::ffi_dispatch::container_alloc_temp(__cprops[{idx}]) }};\n"
        ));
    }

    // Populate input containers (same as native — containers use UeArray/UeSet/UeMap API
    // which already works on wasm32 via ffi_dispatch)
    for (idx, cp_param, cp_dir) in &container_indices {
        if *cp_dir != ParamDirection::In && *cp_dir != ParamDirection::InOut {
            continue;
        }
        let pname = escape_reserved(&to_snake_case(&cp_param.name));
        emit_wasm_container_populate(out, cp_param, *idx, &pname, ctx);
    }

    // Declare scalar output variables
    if let Some(rm) = ret_mapped {
        match rm.ffi_to_rust {
            ConversionKind::ObjectRef => {
                out.push_str("        let mut __scalar_ret: u64 = 0;\n");
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
                let default = super::properties::default_value_for(&rm.rust_ffi_type);
                out.push_str(&format!("        let mut __scalar_ret = {default};\n"));
            }
        }
    }

    // Scalar Out params
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
                    out.push_str(&format!("        let mut {pname}: u64 = 0;\n"));
                }
                ConversionKind::EnumCast => {
                    out.push_str(&format!("        let mut {pname}: {} = 0;\n", mapped.rust_ffi_type));
                }
                _ => {
                    let default = super::properties::default_value_for(&mapped.rust_ffi_type);
                    out.push_str(&format!("        let mut {pname} = {default};\n"));
                }
            }
        }
        if dir == ParamDirection::InOut && !is_container_param(param) {
            let mapped = map_param(param);
            if mapped.ffi_to_rust == ConversionKind::StringUtf8 {
                let pname = escape_reserved(&to_snake_case(&param.name));
                out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
                out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
            }
        }
    }

    // Pre-declare temp buffers for owned struct In params.
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if dir == ParamDirection::In && !is_container_param(param) {
            let mapped = map_param(param);
            if mapped.rust_to_ffi == ConversionKind::StructOpaque
                && is_struct_owned(param.struct_name.as_deref(), ctx)
            {
                let pname = escape_reserved(&to_snake_case(&param.name));
                out.push_str(&format!("        let {pname}_bytes = {pname}.to_bytes();\n"));
            }
        }
    }

    // FFI call via extern import
    out.push_str(&format!(
        "        let __result = unsafe {{ crate::wasm_fn_imports::{}(",
        wasm_fn_name(func_id)
    ));

    if !is_static {
        out.push_str("h.0 as i64, ");
    }

    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            let (idx, _, _) = container_indices.iter()
                .find(|(_, p, _)| std::ptr::eq(*p, param))
                .expect("container param must exist");
            // Container params: base as i64 (NativePtr), prop handle as i64
            out.push_str(&format!(
                "__temp_{idx} as i64, __cprops[{idx}].0 as i64, "
            ));
        } else {
            let pname = escape_reserved(&to_snake_case(&param.name));
            let mapped = map_param(param);
            match dir {
                ParamDirection::In | ParamDirection::InOut => {
                    match mapped.rust_to_ffi {
                        ConversionKind::StringUtf8 => {
                            out.push_str(&format!("{pname}.as_ptr() as i32, {pname}.len() as i32, "));
                            if dir == ParamDirection::InOut {
                                out.push_str(&format!(
                                    "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, &mut {pname}_len as *mut u32 as i32, "
                                ));
                            }
                        }
                        ConversionKind::ObjectRef => {
                            out.push_str(&format!("{pname}.raw_handle().0 as i64, "));
                        }
                        ConversionKind::EnumCast => {
                            out.push_str(&format!("{pname} as {} as i32, ", mapped.rust_ffi_type));
                        }
                        ConversionKind::FName => {
                            out.push_str(&format!("{pname}.0 as i64, "));
                        }
                        ConversionKind::StructOpaque if dir == ParamDirection::In
                            && is_struct_owned(param.struct_name.as_deref(), ctx) =>
                        {
                            // Owned struct: use pre-declared bytes variable.
                            out.push_str(&format!(
                                "{pname}_bytes.as_ptr() as i32, {pname}_bytes.len() as i32, "
                            ));
                        }
                        ConversionKind::StructOpaque if dir == ParamDirection::In => {
                            // Non-owned struct: *const u8 in WASM memory.
                            out.push_str(&format!("{pname} as i32, 0i32, "));
                        }
                        ConversionKind::StructOpaque if dir == ParamDirection::InOut => {
                            out.push_str(&format!("{pname} as i32, 256i32, "));
                        }
                        ConversionKind::Identity if mapped.rust_ffi_type == "bool" => {
                            out.push_str(&format!("{pname} as i32, "));
                        }
                        _ => {
                            let wt = ffi_type_to_wasm(&mapped.rust_ffi_type);
                            out.push_str(&format!("{pname} as {wt}, "));
                        }
                    }
                }
                ParamDirection::Out => {
                    match mapped.ffi_to_rust {
                        ConversionKind::StructOpaque => {
                            out.push_str(&format!(
                                "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, "
                            ));
                        }
                        ConversionKind::StringUtf8 => {
                            out.push_str(&format!(
                                "{pname}_buf.as_mut_ptr() as i32, {pname}_buf.len() as i32, &mut {pname}_len as *mut u32 as i32, "
                            ));
                        }
                        ConversionKind::ObjectRef => {
                            out.push_str(&format!("&mut {pname} as *mut u64 as i32, "));
                        }
                        _ => {
                            out.push_str(&format!(
                                "&mut {pname} as *mut {} as i32, ",
                                mapped.rust_ffi_type
                            ));
                        }
                    }
                }
                ParamDirection::Return => {
                    let rm = ret_mapped.unwrap();
                    match rm.ffi_to_rust {
                        ConversionKind::StringUtf8 => {
                            out.push_str(
                                "__scalar_ret_buf.as_mut_ptr() as i32, __scalar_ret_buf.len() as i32, &mut __scalar_ret_len as *mut u32 as i32, "
                            );
                        }
                        ConversionKind::ObjectRef => {
                            out.push_str("&mut __scalar_ret as *mut u64 as i32, ");
                        }
                        ConversionKind::StructOpaque => {
                            out.push_str(
                                "__scalar_ret_buf.as_mut_ptr() as i32, __scalar_ret_buf.len() as i32, "
                            );
                        }
                        _ => {
                            out.push_str(&format!(
                                "&mut __scalar_ret as *mut {} as i32, ",
                                rm.rust_ffi_type
                            ));
                        }
                    }
                }
            }
        }
    }
    // Remove trailing comma+space
    if out.ends_with(", ") {
        out.truncate(out.len() - 2);
    }
    out.push_str(") };\n");

    // Transmute error code
    out.push_str("        let __result: uika_runtime::UikaErrorCode = unsafe { core::mem::transmute(__result as u32) };\n");

    // Read output containers (same as native — UeArray/UeSet/UeMap work on wasm32)
    for (idx, cp_param, cp_dir) in &container_indices {
        if *cp_dir != ParamDirection::Out && *cp_dir != ParamDirection::Return && *cp_dir != ParamDirection::InOut {
            continue;
        }
        emit_wasm_container_read(out, cp_param, *idx, ctx);
    }

    // Free ALL temps
    out.push_str("        unsafe {\n");
    for (idx, _, _) in &container_indices {
        out.push_str(&format!(
            "            uika_runtime::ffi_dispatch::container_free_temp(__cprops[{idx}], __temp_{idx});\n"
        ));
    }
    out.push_str("        }\n");

    // Assert success
    out.push_str("        uika_runtime::ffi_infallible(__result);\n");

    // Return (assemble scalar + container outputs)
    emit_wasm_container_return(out, return_param, ret_mapped, &container_indices, &func.params, ctx);
}

/// Emit code to populate an input container in wasm32 mode.
/// Uses UeArray/UeSet/UeMap which already work via ffi_dispatch on wasm32.
fn emit_wasm_container_populate(
    out: &mut String,
    param: &ParamInfo,
    idx: usize,
    pname: &str,
    ctx: &CodegenContext,
) {
    let elem_type = super::classes::container_elem_type_str(param, ctx)
        .expect("container element type must be resolvable");

    // On wasm32, __temp_{idx} is a NativePtr (u64), need to construct UObjectHandle from it
    let handle_expr = format!("uika_runtime::UObjectHandle(__temp_{idx})");

    match param.prop_type.as_str() {
        "ArrayProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __arr = uika_runtime::UeArray::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
                 \x20           for __elem in {pname} {{\n\
                 \x20               let _ = __arr.push(__elem);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        "SetProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __set = uika_runtime::UeSet::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
                 \x20           for __elem in {pname} {{\n\
                 \x20               let _ = __set.add(__elem);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        "MapProperty" => {
            out.push_str(&format!(
                "        {{\n\
                 \x20           let __map = uika_runtime::UeMap::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
                 \x20           for (__k, __v) in {pname} {{\n\
                 \x20               let _ = __map.add(__k, __v);\n\
                 \x20           }}\n\
                 \x20       }}\n"
            ));
        }
        _ => {}
    }
}

/// Emit code to read an output container in wasm32 mode.
fn emit_wasm_container_read(
    out: &mut String,
    param: &ParamInfo,
    idx: usize,
    ctx: &CodegenContext,
) {
    let elem_type = super::classes::container_elem_type_str(param, ctx)
        .expect("container element type must be resolvable");

    let handle_expr = format!("uika_runtime::UObjectHandle(__temp_{idx})");

    match param.prop_type.as_str() {
        "ArrayProperty" => {
            out.push_str(&format!(
                "        let __out_{idx} = if __result == uika_runtime::UikaErrorCode::Ok {{\n\
                 \x20           let __arr = uika_runtime::UeArray::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
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
                 \x20           let __set = uika_runtime::UeSet::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
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
                 \x20           let __map = uika_runtime::UeMap::<{elem_type}>::new({handle_expr}, __cprops[{idx}]);\n\
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

/// Emit the return expression for container functions in wasm32 mode.
fn emit_wasm_container_return(
    out: &mut String,
    return_param: Option<&ParamInfo>,
    ret_mapped: Option<&MappedType>,
    container_indices: &[(usize, &ParamInfo, ParamDirection)],
    func_params: &[ParamInfo],
    ctx: &CodegenContext,
) {
    let mut return_parts = Vec::new();

    // Scalar or container return value
    if let Some(rp) = return_param {
        if is_container_param(rp) {
            let (idx, _, _) = container_indices.iter()
                .find(|(_, _, d)| *d == ParamDirection::Return)
                .expect("container return must exist");
            return_parts.push(format!("__out_{idx}"));
        } else if let Some(rm) = ret_mapped {
            match rm.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    out.push_str("        let __scalar_ret_handle = uika_runtime::UObjectHandle(__scalar_ret);\n");
                    return_parts.push("unsafe { uika_runtime::UObjectRef::from_raw(__scalar_ret_handle) }".to_string());
                }
                ConversionKind::StringUtf8 => {
                    out.push_str("        __scalar_ret_buf.truncate(__scalar_ret_len as usize);\n");
                    out.push_str("        let __scalar_str = String::from_utf8_lossy(&__scalar_ret_buf).into_owned();\n");
                    return_parts.push("__scalar_str".to_string());
                }
                ConversionKind::EnumCast => {
                    let rt = &rm.rust_type;
                    let rp_ref = return_param.unwrap();
                    let actual_repr = rp_ref.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&rm.rust_ffi_type);
                    out.push_str(&format!(
                        "        let __scalar_enum = {rt}::from_value(__scalar_ret as {actual_repr}).expect(\"unknown enum value\");\n"
                    ));
                    return_parts.push("__scalar_enum".to_string());
                }
                ConversionKind::StructOpaque => {
                    let rp_ref = return_param.unwrap();
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

    // Out/InOut params
    for param in func_params {
        let dir = type_map::param_direction(param);
        if dir != ParamDirection::Out && dir != ParamDirection::InOut {
            continue;
        }

        if is_container_param(param) {
            let (idx, _, _) = container_indices.iter()
                .find(|(_, p, _)| std::ptr::eq(*p, param))
                .expect("container param must exist");
            return_parts.push(format!("__out_{idx}"));
        } else {
            let mapped = map_param(param);
            if !super::classes::is_scalar_output_returnable(dir, &mapped) {
                continue;
            }
            let pname = escape_reserved(&to_snake_case(&param.name));
            match mapped.ffi_to_rust {
                ConversionKind::ObjectRef => {
                    out.push_str(&format!(
                        "        let {pname}_handle = uika_runtime::UObjectHandle({pname});\n"
                    ));
                    return_parts.push(format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}_handle) }}"));
                }
                ConversionKind::StringUtf8 => {
                    out.push_str(&format!("        {pname}_buf.truncate({pname}_len as usize);\n"));
                    out.push_str(&format!(
                        "        let {pname}_str = String::from_utf8_lossy(&{pname}_buf).into_owned();\n"
                    ));
                    return_parts.push(format!("{pname}_str"));
                }
                ConversionKind::EnumCast => {
                    let rt = &mapped.rust_type;
                    let actual_repr = param.enum_name.as_deref()
                        .and_then(|en| ctx.enum_actual_repr(en))
                        .unwrap_or(&mapped.rust_ffi_type);
                    out.push_str(&format!(
                        "        let {pname}_enum = {rt}::from_value({pname} as {actual_repr}).expect(\"unknown enum value\");\n"
                    ));
                    return_parts.push(format!("{pname}_enum"));
                }
                ConversionKind::StructOpaque => {
                    if is_struct_owned(param.struct_name.as_deref(), ctx) {
                        out.push_str(&format!(
                            "        let {pname}_owned = uika_runtime::OwnedStruct::from_bytes({pname}_buf);\n"
                        ));
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
                    // {pname} is already FNameHandle (declared via Default::default())
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

// ---------------------------------------------------------------------------
// Host function generation (wasm_host_codegen_funcs.rs)
// ---------------------------------------------------------------------------

/// Generate the complete wasm_host_codegen_funcs.rs file content.
/// This file is `include!`'d by uika-wasm-host and registers all codegen
/// functions as wasmtime host functions.
pub fn generate_wasm_host_funcs(func_table: &[FuncEntry], ctx: &CodegenContext) -> String {
    let mut out = String::with_capacity(256 * 1024);
    out.push_str("// Auto-generated host function registration for codegen functions.\n");
    out.push_str("// Do not edit manually.\n\n");

    out.push_str("pub fn register_codegen_host_functions(\n");
    out.push_str("    linker: &mut wasmtime::Linker<HostState>,\n");
    out.push_str(") -> wasmtime::Result<()> {\n");

    for entry in func_table {
        if let Some(code) = generate_single_host_func(entry, ctx) {
            out.push_str(&code);
        }
    }

    out.push_str("    Ok(())\n");
    out.push_str("}\n");
    out
}

/// Remap `uika_runtime::` types to `uika_ffi::` for host function code.
/// The host crate depends on `uika-ffi`, not `uika-runtime`.
fn host_ffi_type(ty: &str) -> String {
    ty.replace("uika_runtime::", "uika_ffi::")
}

/// Generate the linker.func_wrap call for a single function.
/// Returns None if the function has unsupported params.
fn generate_single_host_func(entry: &FuncEntry, ctx: &CodegenContext) -> Option<String> {
    let func = &entry.func;
    let func_id = entry.func_id;
    let is_static = func.is_static || (func.func_flags & FUNC_STATIC != 0);

    // Classify params and check support
    let mut all_mapped: Vec<(&ParamInfo, ParamDirection, MappedType)> = Vec::new();
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            // Check container support
            if dir == ParamDirection::In || dir == ParamDirection::InOut {
                if super::classes::container_param_input_type(param, ctx).is_none() {
                    return None;
                }
            }
            if dir == ParamDirection::Out || dir == ParamDirection::Return || dir == ParamDirection::InOut {
                if super::classes::container_param_output_type(param, ctx).is_none() {
                    return None;
                }
            }
        } else {
            let mapped = map_param(param);
            if !mapped.supported {
                return None;
            }
            all_mapped.push((param, dir, mapped));
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "    // FN_ID={func_id}: {}.{}\n",
        entry.class_name, entry.func_name
    ));

    // Build the WASM parameter list for func_wrap's closure
    let mut closure_params = Vec::new();
    closure_params.push("mut caller: wasmtime::Caller<'_, HostState>".to_string());

    if !is_static {
        closure_params.push("obj: i64".to_string());
    }

    for param in &func.params {
        let dir = type_map::param_direction(param);
        let pname = escape_reserved(&to_snake_case(&param.name));

        if is_container_param(param) {
            closure_params.push(format!("{pname}_base: i64"));
            closure_params.push(format!("{pname}_prop: i64"));
            continue;
        }

        let mapped = map_param(param);
        let slots = param_wasm_slots(param, dir, &mapped);
        for slot in &slots {
            match slot {
                WasmParamSlot::Single(wt) => {
                    closure_params.push(format!("{pname}: {wt}"));
                }
                WasmParamSlot::PtrLen => {
                    closure_params.push(format!("{pname}_ptr: i32"));
                    closure_params.push(format!("{pname}_len: i32"));
                }
                WasmParamSlot::PtrLenOutLen => {
                    closure_params.push(format!("{pname}_buf: i32"));
                    closure_params.push(format!("{pname}_buf_len: i32"));
                    closure_params.push(format!("{pname}_out_len: i32"));
                }
                WasmParamSlot::ScalarOut(_wt) => {
                    closure_params.push(format!("{pname}_out: i32"));
                }
                WasmParamSlot::NativePtr => {
                    closure_params.push(format!("{pname}: i64"));
                }
                WasmParamSlot::ContainerPtrs => {
                    unreachable!();
                }
            }
        }
    }

    // wasmtime func_wrap supports at most 16 closure params (including Caller).
    // Skip functions that exceed this limit.
    if closure_params.len() > 16 {
        return None;
    }

    let closure_params_str = closure_params.join(", ");

    out.push_str(&format!(
        "    linker.func_wrap(\"uika_fn\", \"{}\", |{closure_params_str}| -> i32 {{\n",
        wasm_fn_name(func_id)
    ));

    // Get func_table pointer from API
    out.push_str(&format!(
        "        let __func_ptr = unsafe {{ *((*caller.data().api).func_table.add({func_id} as usize)) }};\n"
    ));

    // Build native FFI call type
    let mut ffi_params = Vec::new();
    if !is_static {
        ffi_params.push("uika_ffi::UObjectHandle".to_string());
    }
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            ffi_params.push("*mut u8".to_string()); // base
            ffi_params.push("*mut u8".to_string()); // prop
        } else {
            let mapped = map_param(param);
            match dir {
                ParamDirection::In | ParamDirection::InOut => {
                    match mapped.rust_to_ffi {
                        ConversionKind::StringUtf8 => {
                            ffi_params.push("*const u8".to_string());
                            ffi_params.push("u32".to_string());
                            if dir == ParamDirection::InOut {
                                ffi_params.push("*mut u8".to_string());
                                ffi_params.push("u32".to_string());
                                ffi_params.push("*mut u32".to_string());
                            }
                        }
                        ConversionKind::ObjectRef => ffi_params.push("uika_ffi::UObjectHandle".to_string()),
                        ConversionKind::EnumCast => ffi_params.push(host_ffi_type(&mapped.rust_ffi_type)),
                        ConversionKind::StructOpaque => {
                            if dir == ParamDirection::InOut {
                                ffi_params.push("*mut u8".to_string());
                            } else {
                                ffi_params.push("*const u8".to_string());
                            }
                        }
                        _ => ffi_params.push(host_ffi_type(&mapped.rust_ffi_type)),
                    }
                }
                ParamDirection::Out | ParamDirection::Return => {
                    match mapped.ffi_to_rust {
                        ConversionKind::StringUtf8 => {
                            ffi_params.push("*mut u8".to_string());
                            ffi_params.push("u32".to_string());
                            ffi_params.push("*mut u32".to_string());
                        }
                        ConversionKind::ObjectRef => ffi_params.push("*mut uika_ffi::UObjectHandle".to_string()),
                        ConversionKind::StructOpaque => ffi_params.push("*mut u8".to_string()),
                        ConversionKind::EnumCast => ffi_params.push(format!("*mut {}", host_ffi_type(&mapped.rust_ffi_type))),
                        _ => ffi_params.push(format!("*mut {}", host_ffi_type(&mapped.rust_ffi_type))),
                    }
                }
            }
        }
    }
    let ffi_params_str = ffi_params.join(", ");

    out.push_str(&format!(
        "        type NativeFn = unsafe extern \"C\" fn({ffi_params_str}) -> uika_ffi::UikaErrorCode;\n\
         \x20       let __native_fn: NativeFn = unsafe {{ std::mem::transmute(__func_ptr) }};\n"
    ));

    // Read WASM buffers into local vecs, set up native params
    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            continue; // Container params pass through as native pointers
        }
        let pname = escape_reserved(&to_snake_case(&param.name));
        let mapped = map_param(param);

        match dir {
            ParamDirection::In => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "        let {pname}_bytes = read_guest_bytes(&caller, {pname}_ptr as u32, {pname}_len as u32);\n"
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!(
                            "        let {pname}_bytes = read_guest_bytes(&caller, {pname}_ptr as u32, {pname}_len as u32);\n"
                        ));
                    }
                    _ => {}
                }
            }
            ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        // Read input string from WASM
                        out.push_str(&format!(
                            "        let {pname}_bytes = read_guest_bytes(&caller, {pname}_ptr as u32, {pname}_len as u32);\n"
                        ));
                        // Prepare output buffer
                        out.push_str(&format!(
                            "        let mut {pname}_out_buf = vec![0u8; {pname}_buf_len as usize];\n\
                             \x20       let mut {pname}_out_written: u32 = 0;\n"
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!(
                            "        let mut {pname}_bytes = read_guest_bytes(&caller, {pname}_ptr as u32, {pname}_len as u32);\n"
                        ));
                    }
                    _ => {}
                }
            }
            ParamDirection::Out => {
                match mapped.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "        let mut {pname}_out_buf = vec![0u8; {pname}_buf_len as usize];\n\
                             \x20       let mut {pname}_out_written: u32 = 0;\n"
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!(
                            "        let mut {pname}_out_buf = vec![0u8; {pname}_len as usize];\n"
                        ));
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!(
                            "        let mut {pname}_val = uika_ffi::UObjectHandle::null();\n"
                        ));
                    }
                    ConversionKind::EnumCast => {
                        out.push_str(&format!(
                            "        let mut {pname}_val: {} = 0;\n", host_ffi_type(&mapped.rust_ffi_type)
                        ));
                    }
                    _ => {
                        let host_ty = host_ffi_type(&mapped.rust_ffi_type);
                        let default = super::properties::default_value_for(&mapped.rust_ffi_type);
                        out.push_str(&format!(
                            "        let mut {pname}_val: {host_ty} = {default};\n"
                        ));
                    }
                }
            }
            ParamDirection::Return => {
                match mapped.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "        let mut {pname}_out_buf = vec![0u8; {pname}_buf_len as usize];\n\
                             \x20       let mut {pname}_out_written: u32 = 0;\n"
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!(
                            "        let mut {pname}_out_buf = vec![0u8; {pname}_len as usize];\n"
                        ));
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!(
                            "        let mut {pname}_val = uika_ffi::UObjectHandle::null();\n"
                        ));
                    }
                    ConversionKind::EnumCast => {
                        out.push_str(&format!(
                            "        let mut {pname}_val: {} = 0;\n", host_ffi_type(&mapped.rust_ffi_type)
                        ));
                    }
                    _ => {
                        let host_ty = host_ffi_type(&mapped.rust_ffi_type);
                        let default = super::properties::default_value_for(&mapped.rust_ffi_type);
                        out.push_str(&format!(
                            "        let mut {pname}_val: {host_ty} = {default};\n"
                        ));
                    }
                }
            }
        }
    }

    // Build the native function call
    out.push_str("        let __err = unsafe { __native_fn(");

    // Object handle
    if !is_static {
        out.push_str("uika_ffi::UObjectHandle(obj as usize as *mut std::ffi::c_void), ");
    }

    // Parameters
    for param in &func.params {
        let dir = type_map::param_direction(param);
        let pname = escape_reserved(&to_snake_case(&param.name));

        if is_container_param(param) {
            out.push_str(&format!(
                "{pname}_base as usize as *mut u8, {pname}_prop as usize as *mut u8, "
            ));
            continue;
        }

        let mapped = map_param(param);
        match dir {
            ParamDirection::In => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!("{pname}_bytes.as_ptr(), {pname}_bytes.len() as u32, "));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!("{pname}_bytes.as_ptr(), "));
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!("uika_ffi::UObjectHandle({pname} as usize as *mut std::ffi::c_void), "));
                    }
                    ConversionKind::FName => {
                        out.push_str(&format!("uika_ffi::FNameHandle({pname} as u64), "));
                    }
                    ConversionKind::EnumCast => {
                        out.push_str(&format!("{pname} as {}, ", host_ffi_type(&mapped.rust_ffi_type)));
                    }
                    ConversionKind::Identity if mapped.rust_ffi_type == "bool" => {
                        out.push_str(&format!("{pname} != 0, "));
                    }
                    _ => {
                        out.push_str(&format!("{pname} as {}, ", host_ffi_type(&mapped.rust_ffi_type)));
                    }
                }
            }
            ParamDirection::InOut => {
                match mapped.rust_to_ffi {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "{pname}_bytes.as_ptr(), {pname}_bytes.len() as u32, \
                             {pname}_out_buf.as_mut_ptr(), {pname}_out_buf.len() as u32, &mut {pname}_out_written, "
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!("{pname}_bytes.as_mut_ptr(), "));
                    }
                    ConversionKind::ObjectRef => {
                        out.push_str(&format!("uika_ffi::UObjectHandle({pname} as usize as *mut std::ffi::c_void), "));
                    }
                    _ => {
                        out.push_str(&format!("{pname} as {}, ", host_ffi_type(&mapped.rust_ffi_type)));
                    }
                }
            }
            ParamDirection::Out => {
                match mapped.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "{pname}_out_buf.as_mut_ptr(), {pname}_out_buf.len() as u32, &mut {pname}_out_written, "
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!("{pname}_out_buf.as_mut_ptr(), "));
                    }
                    _ => {
                        out.push_str(&format!("&mut {pname}_val, "));
                    }
                }
            }
            ParamDirection::Return => {
                match mapped.ffi_to_rust {
                    ConversionKind::StringUtf8 => {
                        out.push_str(&format!(
                            "{pname}_out_buf.as_mut_ptr(), {pname}_out_buf.len() as u32, &mut {pname}_out_written, "
                        ));
                    }
                    ConversionKind::StructOpaque => {
                        out.push_str(&format!("{pname}_out_buf.as_mut_ptr(), "));
                    }
                    _ => {
                        out.push_str(&format!("&mut {pname}_val, "));
                    }
                }
            }
        }
    }

    // Remove trailing comma+space
    if out.ends_with(", ") {
        out.truncate(out.len() - 2);
    }
    out.push_str(") };\n");

    // Write output values back to WASM memory
    out.push_str("        if __err as u32 == 0 {\n");

    for param in &func.params {
        let dir = type_map::param_direction(param);
        if is_container_param(param) {
            continue;
        }
        let pname = escape_reserved(&to_snake_case(&param.name));
        let mapped = map_param(param);

        let is_output = dir == ParamDirection::Out || dir == ParamDirection::Return
            || (dir == ParamDirection::InOut && mapped.ffi_to_rust == ConversionKind::StringUtf8);

        if !is_output {
            if dir == ParamDirection::InOut && mapped.ffi_to_rust == ConversionKind::StructOpaque {
                // Write modified struct back to WASM
                out.push_str(&format!(
                    "            write_guest_bytes(&mut caller, {pname}_ptr as u32, &{pname}_bytes);\n"
                ));
            }
            continue;
        }

        match mapped.ffi_to_rust {
            ConversionKind::StringUtf8 => {
                // Write string data to WASM buffer, write length to out_len pointer
                out.push_str(&format!(
                    "            write_guest_bytes(\n\
                     \x20               &mut caller,\n\
                     \x20               {pname}_buf as u32,\n\
                     \x20               &{pname}_out_buf[..{pname}_out_written as usize],\n\
                     \x20           );\n\
                     \x20           write_guest_bytes(\n\
                     \x20               &mut caller,\n\
                     \x20               {pname}_out_len as u32,\n\
                     \x20               &{pname}_out_written.to_le_bytes(),\n\
                     \x20           );\n"
                ));
            }
            ConversionKind::StructOpaque => {
                out.push_str(&format!(
                    "            write_guest_bytes(&mut caller, {pname}_ptr as u32, &{pname}_out_buf);\n"
                ));
            }
            ConversionKind::ObjectRef => {
                out.push_str(&format!(
                    "            write_guest_bytes(\n\
                     \x20               &mut caller,\n\
                     \x20               {pname}_out as u32,\n\
                     \x20               &({pname}_val.0 as usize as u64).to_le_bytes(),\n\
                     \x20           );\n"
                ));
            }
            ConversionKind::Identity if mapped.rust_ffi_type == "bool" => {
                out.push_str(&format!(
                    "            write_guest_bytes(&mut caller, {pname}_out as u32, &[{pname}_val as u8]);\n"
                ));
            }
            ConversionKind::FName => {
                // FNameHandle is a newtype around u64; write inner .0
                out.push_str(&format!(
                    "            write_guest_bytes(\n\
                     \x20               &mut caller,\n\
                     \x20               {pname}_out as u32,\n\
                     \x20               &{pname}_val.0.to_le_bytes(),\n\
                     \x20           );\n"
                ));
            }
            _ => {
                out.push_str(&format!(
                    "            write_guest_bytes(\n\
                     \x20               &mut caller,\n\
                     \x20               {pname}_out as u32,\n\
                     \x20               &{pname}_val.to_le_bytes(),\n\
                     \x20           );\n"
                ));
            }
        }
    }

    out.push_str("        }\n"); // close if __err == 0
    out.push_str("        __err as u32 as i32\n");
    out.push_str("    })?;\n\n");

    Some(out)
}
