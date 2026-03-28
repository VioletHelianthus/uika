// Delegate codegen: generates typed delegate accessor methods and wrapper structs.
//
// For each delegate property on a class, we generate:
// 1. A trait method returning a typed delegate handle struct
// 2. The delegate handle struct with bind()/add() methods that accept typed closures
//
// The handle struct wraps the UObject owner + FPropertyHandle, and the bind/add
// methods register a closure in the Rust delegate_registry, then call the C++ API.

use crate::context::CodegenContext;
use crate::naming::to_snake_case;
use crate::schema::PropertyInfo;
use crate::type_map::{self, ConversionKind};

/// Information about a delegate property to generate code for.
pub struct DelegateInfo<'a> {
    pub prop: &'a PropertyInfo,
    pub class_name: &'a str,
    /// Rust name for the accessor method (snake_case).
    pub rust_name: String,
    /// Struct name for the delegate wrapper (PascalCase).
    pub struct_name: String,
    /// Whether this is a multicast delegate.
    pub is_multicast: bool,
    /// Parsed delegate parameters.
    pub params: Vec<DelegateParam>,
}

/// A single parameter in a delegate signature.
pub struct DelegateParam {
    pub name: String,
    pub rust_type: String,
    pub conversion: ParamConversion,
}

/// How to read a delegate param from the raw params buffer.
pub enum ParamConversion {
    /// Primitive: read directly as the type (bool, i32, f32, etc.)
    Primitive(String),
    /// Object reference: read UObjectHandle, wrap in UObjectRef<T>.
    ObjectRef(String),
    /// Enum: read underlying repr, convert via from_value.
    Enum { rust_type: String, repr: String },
    /// FName: use read_param FFI to properly pack FName (Editor-safe).
    FName,
    /// String/FText: use read_param FFI to read UTF-8 string.
    String,
    /// Struct: use read_param FFI to read struct bytes, wrap in OwnedStruct<T>.
    Struct { struct_name: String, cpp_name: String },
}

/// Collect delegate properties from a class and resolve their param types.
pub fn collect_delegate_props<'a>(
    props: &'a [PropertyInfo],
    class_name: &'a str,
    ctx: &CodegenContext,
) -> Vec<DelegateInfo<'a>> {
    let mut result = Vec::new();

    for prop in props {
        let mapped = type_map::map_property_type(
            &prop.prop_type,
            prop.class_name.as_deref(),
            prop.struct_name.as_deref(),
            prop.enum_name.as_deref(),
            prop.enum_underlying_type.as_deref(),
            prop.meta_class_name.as_deref(),
            prop.interface_name.as_deref(),
        );
        if !matches!(
            mapped.rust_to_ffi,
            ConversionKind::Delegate | ConversionKind::MulticastDelegate
        ) {
            continue;
        }

        let is_multicast = matches!(mapped.rust_to_ffi, ConversionKind::MulticastDelegate);

        // Parse func_info params
        let func_info = match &prop.func_info {
            Some(fi) => fi,
            None => continue,
        };
        let params_json = match func_info.get("params").and_then(|p| p.as_array()) {
            Some(params) => params,
            None => &Vec::new() as &Vec<serde_json::Value>,
        };

        let mut params = Vec::new();
        let mut all_supported = true;

        for param_value in params_json {
            let param_name = param_value
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let param_type = match param_value.get("type").and_then(|t| t.as_str()) {
                Some(t) => t,
                None => {
                    all_supported = false;
                    break;
                }
            };

            match resolve_delegate_param(param_name, param_type, param_value, ctx) {
                Some(dp) => params.push(dp),
                None => {
                    all_supported = false;
                    break;
                }
            }
        }

        if !all_supported {
            continue;
        }

        let rust_name = to_snake_case(&prop.name);
        let struct_name = format!("{}{}Delegate", class_name, prop.name);

        result.push(DelegateInfo {
            prop,
            class_name,
            rust_name,
            struct_name,
            is_multicast,
            params,
        });
    }

    result
}

fn resolve_delegate_param(
    name: &str,
    prop_type: &str,
    value: &serde_json::Value,
    ctx: &CodegenContext,
) -> Option<DelegateParam> {
    let param_name = to_snake_case(name);

    match prop_type {
        "BoolProperty" => Some(DelegateParam {
            name: param_name,
            rust_type: "bool".into(),
            conversion: ParamConversion::Primitive("bool".into()),
        }),
        "Int8Property" => Some(DelegateParam {
            name: param_name,
            rust_type: "i8".into(),
            conversion: ParamConversion::Primitive("i8".into()),
        }),
        "ByteProperty" => {
            if let Some(en) = value.get("enum_name").and_then(|v| v.as_str()) {
                if ctx.enums.contains_key(en) {
                    Some(DelegateParam {
                        name: param_name,
                        rust_type: en.to_string(),
                        conversion: ParamConversion::Enum {
                            rust_type: en.to_string(),
                            repr: ctx
                                .enum_actual_repr(en)
                                .unwrap_or("u8")
                                .to_string(),
                        },
                    })
                } else {
                    None
                }
            } else {
                Some(DelegateParam {
                    name: param_name,
                    rust_type: "u8".into(),
                    conversion: ParamConversion::Primitive("u8".into()),
                })
            }
        }
        "Int16Property" => prim_param(param_name, "i16"),
        "UInt16Property" => prim_param(param_name, "u16"),
        "IntProperty" => prim_param(param_name, "i32"),
        "UInt32Property" => prim_param(param_name, "u32"),
        "Int64Property" => prim_param(param_name, "i64"),
        "UInt64Property" => prim_param(param_name, "u64"),
        "FloatProperty" => prim_param(param_name, "f32"),
        "DoubleProperty" => prim_param(param_name, "f64"),
        "NameProperty" => Some(DelegateParam {
            name: param_name,
            rust_type: "uika_runtime::FNameHandle".into(),
            conversion: ParamConversion::FName,
        }),
        "StrProperty" | "TextProperty" => Some(DelegateParam {
            name: param_name,
            rust_type: "String".into(),
            conversion: ParamConversion::String,
        }),
        "ObjectProperty" | "ClassProperty" => {
            let cls = value.get("class_name").and_then(|v| v.as_str());
            if let Some(cls) = cls {
                if ctx.classes.contains_key(cls) {
                    Some(DelegateParam {
                        name: param_name,
                        rust_type: format!("uika_runtime::UObjectRef<{cls}>"),
                        conversion: ParamConversion::ObjectRef(cls.to_string()),
                    })
                } else {
                    None
                }
            } else {
                Some(DelegateParam {
                    name: param_name,
                    rust_type: "uika_runtime::UObjectHandle".into(),
                    conversion: ParamConversion::Primitive("uika_runtime::UObjectHandle".into()),
                })
            }
        }
        "EnumProperty" => {
            let en = value.get("enum_name").and_then(|v| v.as_str())?;
            if !ctx.enums.contains_key(en) {
                return None;
            }
            Some(DelegateParam {
                name: param_name,
                rust_type: en.to_string(),
                conversion: ParamConversion::Enum {
                    rust_type: en.to_string(),
                    repr: ctx.enum_actual_repr(en).unwrap_or("u8").to_string(),
                },
            })
        }
        "StructProperty" => {
            let sn = value.get("struct_name").and_then(|v| v.as_str())?;
            let si = ctx.structs.get(sn)?;
            if !si.has_static_struct {
                return None;
            }
            Some(DelegateParam {
                name: param_name,
                rust_type: format!("uika_runtime::OwnedStruct<{}>", si.cpp_name),
                conversion: ParamConversion::Struct {
                    struct_name: sn.to_string(),
                    cpp_name: si.cpp_name.clone(),
                },
            })
        }
        _ => None,
    }
}

fn prim_param(name: String, ty: &str) -> Option<DelegateParam> {
    Some(DelegateParam {
        name,
        rust_type: ty.into(),
        conversion: ParamConversion::Primitive(ty.into()),
    })
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

/// Generate trait method implementations for delegate properties (used as default impls).
pub fn generate_delegate_impls(
    out: &mut String,
    delegates: &[DelegateInfo],
) {
    for d in delegates {
        let rust_name = &d.rust_name;
        let struct_name = &d.struct_name;
        let class_name = d.class_name;
        let prop_name = &d.prop.name;
        let prop_name_len = prop_name.len();
        let byte_lit = format!("b\"{}\\0\"", prop_name);

        out.push_str(&format!(
            "    fn {rust_name}(&self) -> {struct_name} {{\n\
             \x20       static PROP: std::sync::OnceLock<uika_runtime::FPropertyHandle> = std::sync::OnceLock::new();\n\
             \x20       let prop = *PROP.get_or_init(|| unsafe {{\n\
             \x20           uika_runtime::ffi_dispatch::reflection_find_property(\n\
             \x20               {class_name}::static_class(), {byte_lit}.as_ptr(), {prop_name_len}\n\
             \x20           )\n\
             \x20       }});\n\
             \x20       {struct_name} {{ owner: self.handle(), prop }}\n\
             \x20   }}\n\n"
        ));
    }
}

/// Generate delegate wrapper structs with typed bind/add methods.
/// These are emitted at the top of the class file (before the trait).
pub fn generate_delegate_structs(
    out: &mut String,
    delegates: &[DelegateInfo],
    class_name: &str,
) {
    for d in delegates {
        let struct_name = &d.struct_name;
        let is_multicast = d.is_multicast;

        out.push_str(&format!(
            "pub struct {struct_name} {{\n\
             \x20   pub owner: uika_runtime::UObjectHandle,\n\
             \x20   pub prop: uika_runtime::FPropertyHandle,\n\
             }}\n\n"
        ));

        // Build the closure parameter types and extraction code.
        // UHT exports the stripped function name (e.g., "OnEditableTextBoxCommittedEvent"),
        // but UE stores the signature UFunction with "__DelegateSignature" suffix.
        let sig_name_base = d.prop.func_info.as_ref()
            .and_then(|fi| fi.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&d.prop.name);
        let sig_name = format!("{sig_name_base}__DelegateSignature");
        let sig_name_len = sig_name.len();
        let sig_byte_lit = format!("b\"{}\\0\"", sig_name);

        // Build closure param types for the user-facing closure signature
        let callback_params: Vec<String> = d.params.iter().map(|p| p.rust_type.clone()).collect();
        let callback_sig = callback_params.join(", ");

        let method_name = if is_multicast { "add" } else { "bind" };
        let api_fn = if is_multicast { "bind_multicast" } else { "bind_unicast" };

        out.push_str(&format!(
            "impl {struct_name} {{\n"
        ));

        // Generate the bind/add method
        out.push_str(&format!(
            "    pub fn {method_name}(&self, mut callback: impl FnMut({callback_sig}) + Send + 'static) -> uika_runtime::UikaResult<uika_runtime::DelegateBinding> {{\n"
        ));

        // If there are params, we need to resolve offsets + property handles via OnceLock
        if !d.params.is_empty() {
            let n_params = d.params.len();
            out.push_str(&format!(
                "        static PARAM_INFO: std::sync::OnceLock<[(u32, uika_runtime::FPropertyHandle); {n_params}]> = std::sync::OnceLock::new();\n\
                 \x20       let param_info = PARAM_INFO.get_or_init(|| unsafe {{\n\
                 \x20           let sig_func = uika_runtime::ffi_dispatch::reflection_find_function_by_class(\n\
                 \x20               {class_name}::static_class(),\n\
                 \x20               {sig_byte_lit}.as_ptr(), {sig_name_len});\n\
                 \x20           [\n"
            ));

            for p in &d.params {
                let param_ue_name = &d.prop.func_info.as_ref()
                    .and_then(|fi| fi.get("params"))
                    .and_then(|ps| ps.as_array())
                    .and_then(|arr| arr.iter().find(|v| {
                        v.get("name").and_then(|n| n.as_str()).map(|n| to_snake_case(n)) == Some(p.name.clone())
                    }))
                    .and_then(|v| v.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or(&p.name);
                let pname_len = param_ue_name.len();
                let pname_lit = format!("b\"{}\\0\"", param_ue_name);
                out.push_str(&format!(
                    "                {{\n\
                     \x20                   let param_prop = uika_runtime::ffi_dispatch::reflection_get_function_param(\n\
                     \x20                       sig_func, {pname_lit}.as_ptr(), {pname_len});\n\
                     \x20                   (uika_runtime::ffi_dispatch::reflection_get_property_offset(param_prop), param_prop)\n\
                     \x20               }},\n"
                ));
            }

            out.push_str(
                "            ]\n\
                 \x20       });\n\
                 \x20       #[allow(unused_variables)] let param_info = param_info;\n"
            );
        }

        // Build the closure wrapper that extracts typed params from NativePtr
        if d.params.is_empty() {
            out.push_str(&format!(
                "        let owner = self.owner;\n\
                 \x20       let prop = self.prop;\n\
                 \x20       uika_runtime::delegate_registry::{api_fn}(owner, prop, move |_params: uika_runtime::ffi_dispatch::NativePtr| {{\n\
                 \x20           callback();\n\
                 \x20       }})\n\
                 \x20   }}\n"
            ));
            out.push_str("}\n\n");
            continue;
        }

        out.push_str(&format!(
            "        let owner = self.owner;\n\
             \x20       let prop = self.prop;\n\
             \x20       uika_runtime::delegate_registry::{api_fn}(owner, prop, move |params: uika_runtime::ffi_dispatch::NativePtr| {{\n\
             \x20           unsafe {{\n"
        ));

        // Extract each parameter
        for (i, p) in d.params.iter().enumerate() {
            let var_name = &p.name;
            match &p.conversion {
                ParamConversion::Primitive(ty) => {
                    out.push_str(&format!(
                        "                let {var_name} = uika_runtime::ffi_dispatch::native_mem_read::<{ty}>(params, param_info[{i}].0 as usize);\n"
                    ));
                }
                ParamConversion::ObjectRef(_cls) => {
                    out.push_str(&format!(
                        "                let {var_name} = uika_runtime::UObjectRef::from_raw(\n\
                         \x20                   uika_runtime::ffi_dispatch::native_mem_read::<uika_runtime::UObjectHandle>(params, param_info[{i}].0 as usize)\n\
                         \x20               );\n"
                    ));
                }
                ParamConversion::Enum { rust_type, repr } => {
                    out.push_str(&format!(
                        "                let __raw_{var_name} = uika_runtime::ffi_dispatch::native_mem_read::<{repr}>(params, param_info[{i}].0 as usize);\n\
                         \x20               let {var_name} = {rust_type}::from_value(__raw_{var_name}).unwrap_or_else(|| std::mem::transmute(__raw_{var_name}));\n"
                    ));
                }
                ParamConversion::FName => {
                    out.push_str(&format!(
                        "                let {var_name} = {{\n\
                         \x20                   let mut __buf = [0u8; 8];\n\
                         \x20                   let mut __written: u32 = 0;\n\
                         \x20                   uika_runtime::ffi_infallible(uika_runtime::ffi_dispatch::delegate_read_param(\n\
                         \x20                       param_info[{i}].1,\n\
                         \x20                       params,\n\
                         \x20                       param_info[{i}].0,\n\
                         \x20                       __buf.as_mut_ptr(),\n\
                         \x20                       8,\n\
                         \x20                       &mut __written,\n\
                         \x20                   ));\n\
                         \x20                   uika_runtime::FNameHandle(u64::from_ne_bytes(__buf))\n\
                         \x20               }};\n"
                    ));
                }
                ParamConversion::String => {
                    out.push_str(&format!(
                        "                let {var_name} = {{\n\
                         \x20                   let mut __buf = vec![0u8; 260];\n\
                         \x20                   let mut __written: u32 = 0;\n\
                         \x20                   let __err = uika_runtime::ffi_dispatch::delegate_read_param(\n\
                         \x20                       param_info[{i}].1,\n\
                         \x20                       params,\n\
                         \x20                       param_info[{i}].0,\n\
                         \x20                       __buf.as_mut_ptr(),\n\
                         \x20                       __buf.len() as u32,\n\
                         \x20                       &mut __written,\n\
                         \x20                   );\n\
                         \x20                   if __err == uika_runtime::UikaErrorCode::BufferTooSmall && __written > 0 {{\n\
                         \x20                       __buf.resize(__written as usize, 0);\n\
                         \x20                       uika_runtime::ffi_infallible(uika_runtime::ffi_dispatch::delegate_read_param(\n\
                         \x20                           param_info[{i}].1,\n\
                         \x20                           params,\n\
                         \x20                           param_info[{i}].0,\n\
                         \x20                           __buf.as_mut_ptr(),\n\
                         \x20                           __buf.len() as u32,\n\
                         \x20                           &mut __written,\n\
                         \x20                       ));\n\
                         \x20                   }}\n\
                         \x20                   if __written >= 4 {{\n\
                         \x20                       let __slen = u32::from_ne_bytes([__buf[0], __buf[1], __buf[2], __buf[3]]) as usize;\n\
                         \x20                       String::from_utf8_lossy(&__buf[4..4 + __slen]).into_owned()\n\
                         \x20                   }} else {{\n\
                         \x20                       String::new()\n\
                         \x20                   }}\n\
                         \x20               }};\n"
                    ));
                }
                ParamConversion::Struct { cpp_name, .. } => {
                    out.push_str(&format!(
                        "                let {var_name} = {{\n\
                         \x20                   let __size = uika_runtime::ffi_dispatch::reflection_get_property_size(param_info[{i}].1) as usize;\n\
                         \x20                   let mut __buf = vec![0u8; __size];\n\
                         \x20                   let mut __written: u32 = 0;\n\
                         \x20                   uika_runtime::ffi_infallible(uika_runtime::ffi_dispatch::delegate_read_param(\n\
                         \x20                       param_info[{i}].1,\n\
                         \x20                       params,\n\
                         \x20                       param_info[{i}].0,\n\
                         \x20                       __buf.as_mut_ptr(),\n\
                         \x20                       __size as u32,\n\
                         \x20                       &mut __written,\n\
                         \x20                   ));\n\
                         \x20                   uika_runtime::OwnedStruct::<{cpp_name}>::from_bytes(__buf)\n\
                         \x20               }};\n"
                    ));
                }
            }
        }

        // Call the user's callback with extracted params
        let param_names: Vec<&str> = d.params.iter().map(|p| p.name.as_str()).collect();
        let call_args = param_names.join(", ");
        out.push_str(&format!(
            "                callback({call_args});\n\
             \x20           }}\n\
             \x20       }})\n\
             \x20   }}\n"
        ));

        out.push_str("}\n\n");
    }
}
