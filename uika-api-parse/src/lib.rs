//! Parses `uika-ffi/src/api_table.rs` to extract sub-table function signatures.
//!
//! Used as a build-dependency by `uika-wasm-host` (host function generation)
//! and `uika-runtime` (ffi dispatch generation). Not a runtime dependency.

use syn::{File, Item, ItemStruct, Fields, Type, BareFnArg};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A parsed sub-table (e.g., `UikaCoreApi`).
#[derive(Debug, Clone)]
pub struct SubTable {
    /// Original struct name, e.g., `"UikaCoreApi"`.
    pub struct_name: String,
    /// Derived module name, e.g., `"core"`.
    pub module_name: String,
    /// Functions in declaration order.
    pub functions: Vec<ApiFn>,
}

/// A single function pointer from a sub-table struct.
#[derive(Debug, Clone)]
pub struct ApiFn {
    /// Field name, e.g., `"is_valid"`.
    pub name: String,
    /// Parameters (in order, with names).
    pub params: Vec<ApiParam>,
    /// Return type (None if `-> ()` or no return).
    pub return_type: Option<ApiType>,
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct ApiParam {
    pub name: String,
    pub ty: ApiType,
}

/// Simplified type representation covering all types used in api_table.rs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiType {
    // Pointer-based handles (cfg-gated: *mut c_void on native, u64 on wasm32)
    UObjectHandle,
    UClassHandle,
    FPropertyHandle,
    UFunctionHandle,
    UStructHandle,
    // Value handles (always u64 / struct)
    FNameHandle,
    FWeakObjectHandle,
    // Error code
    UikaErrorCode,
    // Primitives
    Bool,
    U8,
    U16,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    // Raw pointers
    Ptr { mutability: Mutability, pointee: Box<ApiType> },
    // Named struct pointer (e.g., *const UikaReifyPropExtra)
    NamedStructPtr { mutability: Mutability, name: String },
    // c_void pointer (raw opaque pointer, e.g., *mut c_void in func_table)
    CVoidPtr { mutability: Mutability },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Const,
    Mut,
}

// ---------------------------------------------------------------------------
// Parameter classification for WASM bridge generation
// ---------------------------------------------------------------------------

/// How a parameter should be handled across the WASM boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamClass {
    /// Simple scalar value: pass by value (i32/i64/f32/f64).
    Scalar,
    /// Pointer handle → i64, reconstructed on host side.
    PtrHandle,
    /// Value handle (FNameHandle/FWeakObjectHandle) → i64, direct pass.
    ValueHandle,
    /// Error code → i32.
    ErrorCode,
    /// Pointer+length pair: WASM memory buffer read.
    WasmBufRead { len_param_index: usize },
    /// Pointer+length pair: WASM memory buffer write.
    WasmBufWrite { len_param_index: usize },
    /// Length parameter consumed by a preceding buffer pointer.
    BufLen,
    /// Scalar out-pointer (e.g., *mut bool): pass as i32 WASM memory offset.
    ScalarOut,
    /// Handle out-pointer (e.g., *mut UObjectHandle): pass as i32 WASM memory offset.
    HandleOut,
    /// Native pointer pass-through: pass as opaque i64.
    NativePtr,
    /// Named struct pointer: pass as opaque i64 (native pointer).
    NamedStructPtr,
}

/// Override configuration for native pointer detection.
#[derive(Debug, Clone, Default)]
pub struct NativePtrOverrides {
    /// Maps module_name → list of function names whose `*const u8, u32` pairs
    /// are actually native pointers (not WASM memory buffers).
    pub overrides: std::collections::HashMap<String, Vec<String>>,
}

impl NativePtrOverrides {
    /// Parse from TOML content.
    pub fn from_toml(content: &str) -> Self {
        // Simple manual TOML parsing (avoid adding a toml dependency to this crate).
        // Format:
        //   [overrides]
        //   module_name = ["fn1", "fn2"]
        let mut result = NativePtrOverrides::default();
        let mut in_overrides = false;

        for line in content.lines() {
            let line = line.trim();
            if line == "[overrides]" {
                in_overrides = true;
                continue;
            }
            if line.starts_with('[') {
                in_overrides = false;
                continue;
            }
            if !in_overrides || line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Parse: key = ["val1", "val2"]
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim().to_string();
                let val = val.trim();
                // Strip brackets and parse quoted strings
                let val = val.trim_start_matches('[').trim_end_matches(']');
                let fns: Vec<String> = val
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !fns.is_empty() {
                    result.overrides.insert(key, fns);
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Main parse function
// ---------------------------------------------------------------------------

/// Parse `api_table.rs` source and extract all sub-table definitions.
///
/// Looks for `#[repr(C)]` structs named `Uika*Api` that contain function pointer
/// fields (`pub field_name: unsafe extern "C" fn(...) -> ...`).
pub fn parse_api_table(source: &str) -> Vec<SubTable> {
    let file: File = syn::parse_str(source).expect("Failed to parse api_table.rs");
    let mut tables = Vec::new();

    for item in &file.items {
        if let Item::Struct(s) = item {
            let name = s.ident.to_string();
            // Only process Uika*Api structs (skip UikaApiTable itself)
            if name.starts_with("Uika") && name.ends_with("Api") && name != "UikaApiTable" {
                if let Some(table) = parse_sub_table(s) {
                    tables.push(table);
                }
            }
        }
    }

    tables
}

/// Classify all parameters in all functions of a sub-table for WASM bridging.
pub fn classify_params(
    func: &ApiFn,
    module_name: &str,
    overrides: &NativePtrOverrides,
) -> Vec<ParamClass> {
    let is_override_fn = overrides
        .overrides
        .get(module_name)
        .map_or(false, |fns| fns.contains(&func.name));

    let params = &func.params;
    let mut classes = vec![ParamClass::Scalar; params.len()];

    let mut i = 0;
    while i < params.len() {
        let ty = &params[i].ty;

        match ty {
            // Pointer handles → i64
            ApiType::UObjectHandle
            | ApiType::UClassHandle
            | ApiType::FPropertyHandle
            | ApiType::UFunctionHandle
            | ApiType::UStructHandle => {
                classes[i] = ParamClass::PtrHandle;
            }

            // Value handles → i64
            ApiType::FNameHandle | ApiType::FWeakObjectHandle => {
                classes[i] = ParamClass::ValueHandle;
            }

            // Error code (shouldn't appear as param, but handle it)
            ApiType::UikaErrorCode => {
                classes[i] = ParamClass::ErrorCode;
            }

            // Named struct pointer → native pointer pass-through
            ApiType::NamedStructPtr { .. } => {
                classes[i] = ParamClass::NamedStructPtr;
            }

            // c_void pointer → native pointer
            ApiType::CVoidPtr { .. } => {
                classes[i] = ParamClass::NativePtr;
            }

            // Pointer to handle → handle out param (WASM memory offset)
            ApiType::Ptr { pointee, .. } if is_handle_type(pointee) => {
                classes[i] = ParamClass::HandleOut;
            }

            // *const u8 or *mut u8 — check for buffer pattern BEFORE scalar out
            // (u8 is a scalar type, so this must come first)
            ApiType::Ptr { mutability, pointee } if **pointee == ApiType::U8 => {
                let has_len_next = i + 1 < params.len()
                    && matches!(params[i + 1].ty, ApiType::U32);

                if has_len_next && !is_override_fn {
                    // Buffer pattern: ptr + len
                    match mutability {
                        Mutability::Const => {
                            classes[i] = ParamClass::WasmBufRead { len_param_index: i + 1 };
                        }
                        Mutability::Mut => {
                            classes[i] = ParamClass::WasmBufWrite { len_param_index: i + 1 };
                        }
                    }
                    classes[i + 1] = ParamClass::BufLen;
                    i += 2;
                    continue;
                } else if *mutability == Mutability::Mut
                    && i > 0
                    && matches!(params[i - 1].ty, ApiType::FPropertyHandle)
                    && params.first().map_or(false, |p| matches!(p.ty, ApiType::UObjectHandle))
                {
                    // *mut u8 after (obj, prop) pattern without length →
                    // scalar out (e.g., property.get_u8)
                    // Note: without obj first, it's a native pointer (e.g., container.free_temp)
                    classes[i] = ParamClass::ScalarOut;
                } else {
                    // Standalone *mut u8 / *const u8 without length → native pointer
                    classes[i] = ParamClass::NativePtr;
                }
            }

            // Scalar out-pointer (*mut bool, *mut i32, *mut u32, *mut f32, etc.)
            ApiType::Ptr { mutability: Mutability::Mut, pointee }
                if is_scalar_type(pointee) =>
            {
                classes[i] = ParamClass::ScalarOut;
            }

            // Any other pointer type → native pointer pass-through
            ApiType::Ptr { .. } => {
                classes[i] = ParamClass::NativePtr;
            }

            // Primitives → scalar
            _ => {
                classes[i] = ParamClass::Scalar;
            }
        }

        i += 1;
    }

    classes
}

/// Classify the return type of a function for WASM bridging.
pub fn classify_return(ret: &Option<ApiType>) -> ParamClass {
    match ret {
        None => ParamClass::Scalar, // void → no return
        Some(ty) => match ty {
            ApiType::UObjectHandle
            | ApiType::UClassHandle
            | ApiType::FPropertyHandle
            | ApiType::UFunctionHandle
            | ApiType::UStructHandle => ParamClass::PtrHandle,
            ApiType::FNameHandle | ApiType::FWeakObjectHandle => ParamClass::ValueHandle,
            ApiType::UikaErrorCode => ParamClass::ErrorCode,
            ApiType::Bool | ApiType::U8 | ApiType::U16 | ApiType::U32
            | ApiType::I32 | ApiType::U64 | ApiType::I64
            | ApiType::F32 | ApiType::F64 => ParamClass::Scalar,
            ApiType::Ptr { mutability: Mutability::Mut, pointee } if **pointee == ApiType::U8 => {
                ParamClass::NativePtr // e.g., alloc_params returns *mut u8
            }
            _ => ParamClass::NativePtr,
        },
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_handle_type(ty: &ApiType) -> bool {
    matches!(
        ty,
        ApiType::UObjectHandle
            | ApiType::UClassHandle
            | ApiType::FPropertyHandle
            | ApiType::UFunctionHandle
            | ApiType::UStructHandle
            | ApiType::FNameHandle
    )
}

fn is_scalar_type(ty: &ApiType) -> bool {
    matches!(
        ty,
        ApiType::Bool
            | ApiType::U8
            | ApiType::U16
            | ApiType::U32
            | ApiType::U64
            | ApiType::I32
            | ApiType::I64
            | ApiType::F32
            | ApiType::F64
    )
}

fn parse_sub_table(s: &ItemStruct) -> Option<SubTable> {
    let struct_name = s.ident.to_string();
    let module_name = derive_module_name(&struct_name);

    let fields = match &s.fields {
        Fields::Named(f) => &f.named,
        _ => return None,
    };

    let mut functions = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref()?.to_string();

        // Skip non-function fields (e.g., `_opaque: u8`)
        if field_name.starts_with('_') {
            continue;
        }

        // Extract the function pointer type
        if let Some(api_fn) = parse_fn_field(&field_name, &field.ty) {
            functions.push(api_fn);
        }
    }

    if functions.is_empty() {
        return None;
    }

    Some(SubTable {
        struct_name,
        module_name,
        functions,
    })
}

/// `UikaCoreApi` → `"core"`, `UikaPropertyApi` → `"property"`, etc.
fn derive_module_name(struct_name: &str) -> String {
    let name = struct_name
        .strip_prefix("Uika")
        .unwrap_or(struct_name)
        .strip_suffix("Api")
        .unwrap_or(struct_name);
    to_snake_case(name)
}

fn to_snake_case(s: &str) -> String {
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

fn parse_fn_field(name: &str, ty: &Type) -> Option<ApiFn> {
    // Match: unsafe extern "C" fn(args...) -> ReturnType
    let bare_fn = match ty {
        Type::BareFn(f) => f,
        _ => return None,
    };

    let params: Vec<ApiParam> = bare_fn
        .inputs
        .iter()
        .filter_map(|arg| parse_bare_fn_arg(arg))
        .collect();

    let return_type = match &bare_fn.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => parse_api_type(ty),
    };

    Some(ApiFn {
        name: name.to_string(),
        params,
        return_type,
    })
}

fn parse_bare_fn_arg(arg: &BareFnArg) -> Option<ApiParam> {
    let name = arg
        .name
        .as_ref()
        .map(|(ident, _)| ident.to_string())
        .unwrap_or_else(|| "_".to_string());
    let ty = parse_api_type(&arg.ty)?;
    Some(ApiParam { name, ty })
}

fn parse_api_type(ty: &Type) -> Option<ApiType> {
    match ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let name = seg.ident.to_string();
            match name.as_str() {
                "UObjectHandle" => Some(ApiType::UObjectHandle),
                "UClassHandle" => Some(ApiType::UClassHandle),
                "FPropertyHandle" => Some(ApiType::FPropertyHandle),
                "UFunctionHandle" => Some(ApiType::UFunctionHandle),
                "UStructHandle" => Some(ApiType::UStructHandle),
                "FNameHandle" => Some(ApiType::FNameHandle),
                "FWeakObjectHandle" => Some(ApiType::FWeakObjectHandle),
                "UikaErrorCode" => Some(ApiType::UikaErrorCode),
                "bool" => Some(ApiType::Bool),
                "u8" => Some(ApiType::U8),
                "u16" => Some(ApiType::U16),
                "u32" => Some(ApiType::U32),
                "u64" => Some(ApiType::U64),
                "i32" => Some(ApiType::I32),
                "i64" => Some(ApiType::I64),
                "f32" => Some(ApiType::F32),
                "f64" => Some(ApiType::F64),
                "c_void" => Some(ApiType::CVoidPtr { mutability: Mutability::Mut }), // bare c_void shouldn't appear
                _ => None, // Unknown named type
            }
        }
        Type::Ptr(ptr) => {
            let mutability = if ptr.mutability.is_some() {
                Mutability::Mut
            } else {
                Mutability::Const
            };

            // Check for c_void
            if let Type::Path(tp) = &*ptr.elem {
                let seg = tp.path.segments.last();
                if let Some(seg) = seg {
                    if seg.ident == "c_void" {
                        return Some(ApiType::CVoidPtr { mutability });
                    }
                    // Named struct pointer (e.g., *const UikaReifyPropExtra)
                    let name = seg.ident.to_string();
                    if name.starts_with("Uika") || name.starts_with('F') || name.starts_with('U') {
                        // Check if it's a known handle type
                        if let Some(inner) = parse_api_type(&ptr.elem) {
                            if is_handle_type(&inner) || is_scalar_type(&inner) {
                                return Some(ApiType::Ptr {
                                    mutability,
                                    pointee: Box::new(inner),
                                });
                            }
                        }
                        // Otherwise it's a named struct pointer
                        return Some(ApiType::NamedStructPtr { mutability, name });
                    }
                }
            }

            // Standard pointer to parsed type
            let pointee = parse_api_type(&ptr.elem)?;
            Some(ApiType::Ptr {
                mutability,
                pointee: Box::new(pointee),
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_module_name() {
        assert_eq!(derive_module_name("UikaCoreApi"), "core");
        assert_eq!(derive_module_name("UikaPropertyApi"), "property");
        assert_eq!(derive_module_name("UikaReflectionApi"), "reflection");
        assert_eq!(derive_module_name("UikaContainerApi"), "container");
        assert_eq!(derive_module_name("UikaDelegateApi"), "delegate");
        assert_eq!(derive_module_name("UikaLifecycleApi"), "lifecycle");
        assert_eq!(derive_module_name("UikaReifyApi"), "reify");
        assert_eq!(derive_module_name("UikaWorldApi"), "world");
        assert_eq!(derive_module_name("UikaLoggingApi"), "logging");
        assert_eq!(derive_module_name("UikaMemoryApi"), "memory");
    }

    #[test]
    fn test_parse_simple_fn() {
        let src = r#"
            #[repr(C)]
            pub struct UikaCoreApi {
                pub is_valid: unsafe extern "C" fn(obj: UObjectHandle) -> bool,
                pub get_class: unsafe extern "C" fn(obj: UObjectHandle) -> UClassHandle,
            }
        "#;
        let tables = parse_api_table(src);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].struct_name, "UikaCoreApi");
        assert_eq!(tables[0].module_name, "core");
        assert_eq!(tables[0].functions.len(), 2);

        let f0 = &tables[0].functions[0];
        assert_eq!(f0.name, "is_valid");
        assert_eq!(f0.params.len(), 1);
        assert_eq!(f0.params[0].ty, ApiType::UObjectHandle);
        assert_eq!(f0.return_type, Some(ApiType::Bool));

        let f1 = &tables[0].functions[1];
        assert_eq!(f1.name, "get_class");
        assert_eq!(f1.return_type, Some(ApiType::UClassHandle));
    }

    #[test]
    fn test_parse_buffer_params() {
        let src = r#"
            #[repr(C)]
            pub struct UikaCoreApi {
                pub get_name: unsafe extern "C" fn(
                    obj: UObjectHandle,
                    buf: *mut u8,
                    buf_len: u32,
                    out_len: *mut u32,
                ) -> UikaErrorCode,
            }
        "#;
        let tables = parse_api_table(src);
        let func = &tables[0].functions[0];
        assert_eq!(func.params.len(), 4);

        let classes = classify_params(func, "core", &NativePtrOverrides::default());
        assert_eq!(classes[0], ParamClass::PtrHandle); // obj
        assert_eq!(classes[1], ParamClass::WasmBufWrite { len_param_index: 2 }); // buf
        assert_eq!(classes[2], ParamClass::BufLen); // buf_len
        assert_eq!(classes[3], ParamClass::ScalarOut); // out_len
    }

    #[test]
    fn test_native_ptr_override() {
        let src = r#"
            #[repr(C)]
            pub struct UikaWorldApi {
                pub spawn_actor: unsafe extern "C" fn(
                    world: UObjectHandle,
                    class: UClassHandle,
                    transform_buf: *const u8,
                    transform_size: u32,
                    owner: UObjectHandle,
                ) -> UObjectHandle,
            }
        "#;
        let tables = parse_api_table(src);
        let func = &tables[0].functions[0];

        // Without override: treated as WASM buffer
        let classes = classify_params(func, "world", &NativePtrOverrides::default());
        assert_eq!(classes[2], ParamClass::WasmBufRead { len_param_index: 3 });

        // With override: treated as native pointer
        let mut overrides = NativePtrOverrides::default();
        overrides.overrides.insert("world".to_string(), vec!["spawn_actor".to_string()]);
        let classes = classify_params(func, "world", &overrides);
        assert_eq!(classes[2], ParamClass::NativePtr);
    }

    #[test]
    fn test_parse_real_api_table() {
        let src = include_str!("../../uika-ffi/src/api_table.rs");
        let tables = parse_api_table(src);

        let names: Vec<&str> = tables.iter().map(|t| t.module_name.as_str()).collect();
        assert!(names.contains(&"core"), "missing core, got: {names:?}");
        assert!(names.contains(&"property"), "missing property");
        assert!(names.contains(&"reflection"), "missing reflection");
        assert!(names.contains(&"container"), "missing container");
        assert!(names.contains(&"delegate"), "missing delegate");
        assert!(names.contains(&"lifecycle"), "missing lifecycle");
        assert!(names.contains(&"reify"), "missing reify");
        assert!(names.contains(&"world"), "missing world");
        assert!(names.contains(&"logging"), "missing logging");

        // Count total functions
        let total: usize = tables.iter().map(|t| t.functions.len()).sum();
        assert!(total > 80, "expected >80 functions, got {total}");

        // Spot check: core.is_valid
        let core = tables.iter().find(|t| t.module_name == "core").unwrap();
        let is_valid = core.functions.iter().find(|f| f.name == "is_valid").unwrap();
        assert_eq!(is_valid.params.len(), 1);
        assert_eq!(is_valid.params[0].ty, ApiType::UObjectHandle);
        assert_eq!(is_valid.return_type, Some(ApiType::Bool));

        // Spot check: reflection.alloc_params → returns *mut u8 (native pointer)
        let refl = tables.iter().find(|t| t.module_name == "reflection").unwrap();
        let alloc = refl.functions.iter().find(|f| f.name == "alloc_params").unwrap();
        let ret_class = classify_return(&alloc.return_type);
        assert_eq!(ret_class, ParamClass::NativePtr);

        // Spot check: reify.add_property has *const UikaReifyPropExtra
        let reify = tables.iter().find(|t| t.module_name == "reify").unwrap();
        let add_prop = reify.functions.iter().find(|f| f.name == "add_property").unwrap();
        let extra_param = add_prop.params.iter().find(|p| p.name == "extra").unwrap();
        assert!(matches!(extra_param.ty, ApiType::NamedStructPtr { .. }));
    }

    #[test]
    fn test_parse_overrides_toml() {
        let toml = r#"
[overrides]
world = ["spawn_actor", "spawn_actor_deferred", "finish_spawning"]
"#;
        let overrides = NativePtrOverrides::from_toml(toml);
        assert_eq!(
            overrides.overrides.get("world").unwrap(),
            &vec!["spawn_actor".to_string(), "spawn_actor_deferred".to_string(), "finish_spawning".to_string()]
        );
    }
}
