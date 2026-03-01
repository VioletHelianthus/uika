// Shared parameter handling helpers for native (classes.rs) and WASM (wasm_gen.rs) codegen.
//
// Extracts duplicated logic for output variable declarations, return value
// conversion, and return expression formatting.

use crate::context::CodegenContext;
use crate::naming::{escape_reserved, to_snake_case};
use crate::schema::*;
use crate::type_map::{ConversionKind, MappedType};

use super::classes::is_struct_owned;
use super::properties;

/// Target platform for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Native,
    Wasm32,
}

// ---------------------------------------------------------------------------
// Output variable declarations
// ---------------------------------------------------------------------------

/// Emit `let mut` declarations for a scalar Out parameter.
pub fn emit_out_param_var_decl(
    out: &mut String,
    param: &ParamInfo,
    mapped: &MappedType,
    platform: Platform,
) {
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
            match platform {
                Platform::Native => {
                    out.push_str(&format!(
                        "        let mut {pname} = uika_runtime::UObjectHandle::null();\n"
                    ));
                }
                Platform::Wasm32 => {
                    out.push_str(&format!("        let mut {pname}: u64 = 0;\n"));
                }
            }
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

/// Emit output buffer declarations for InOut string/text parameters.
pub fn emit_inout_string_buf_decl(out: &mut String, param: &ParamInfo, mapped: &MappedType) {
    if mapped.ffi_to_rust == ConversionKind::StringUtf8 {
        let pname = escape_reserved(&to_snake_case(&param.name));
        out.push_str(&format!("        let mut {pname}_buf = vec![0u8; 512];\n"));
        out.push_str(&format!("        let mut {pname}_len: u32 = 0;\n"));
    }
}

// ---------------------------------------------------------------------------
// Return value conversion (Out/InOut parameters)
// ---------------------------------------------------------------------------

/// Emit return conversion for a scalar Out/InOut parameter.
/// Returns the expression string to include in the return tuple.
pub fn emit_out_param_conversion(
    out: &mut String,
    param: &ParamInfo,
    mapped: &MappedType,
    platform: Platform,
    ctx: &CodegenContext,
) -> String {
    let pname = escape_reserved(&to_snake_case(&param.name));
    match mapped.ffi_to_rust {
        ConversionKind::ObjectRef => {
            match platform {
                Platform::Native => {
                    format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}) }}")
                }
                Platform::Wasm32 => {
                    out.push_str(&format!(
                        "        let {pname}_handle = uika_runtime::UObjectHandle({pname});\n"
                    ));
                    format!("unsafe {{ uika_runtime::UObjectRef::from_raw({pname}_handle) }}")
                }
            }
        }
        ConversionKind::StringUtf8 => {
            out.push_str(&format!("        {pname}_buf.truncate({pname}_len as usize);\n"));
            out.push_str(&format!(
                "        let {pname}_str = String::from_utf8_lossy(&{pname}_buf).into_owned();\n"
            ));
            format!("{pname}_str")
        }
        ConversionKind::EnumCast => {
            let rt = &mapped.rust_type;
            let actual_repr = param.enum_name.as_deref()
                .and_then(|en| ctx.enum_actual_repr(en))
                .unwrap_or(&mapped.rust_ffi_type);
            out.push_str(&format!(
                "        let {pname}_enum = {rt}::from_value({pname} as {actual_repr}).expect(\"unknown enum value\");\n"
            ));
            format!("{pname}_enum")
        }
        ConversionKind::StructOpaque => {
            if is_struct_owned(param.struct_name.as_deref(), ctx) {
                out.push_str(&format!(
                    "        let {pname}_owned = uika_runtime::OwnedStruct::from_bytes({pname}_buf);\n"
                ));
                format!("{pname}_owned")
            } else {
                out.push_str(&format!("        let {pname}_ptr = {pname}_buf.as_ptr();\n"));
                out.push_str(&format!("        std::mem::forget({pname}_buf);\n"));
                format!("{pname}_ptr")
            }
        }
        ConversionKind::IntCast => {
            let rt = &mapped.rust_type;
            format!("{pname} as {rt}")
        }
        ConversionKind::FName => {
            pname.to_string()
        }
        _ => {
            pname.to_string()
        }
    }
}

/// Emit the final return expression from a list of return parts.
pub fn emit_return_expr(out: &mut String, return_parts: &[String]) {
    match return_parts.len() {
        0 => {},
        1 => out.push_str(&format!("        {}\n", return_parts[0])),
        _ => out.push_str(&format!("        ({})\n", return_parts.join(", "))),
    }
}
