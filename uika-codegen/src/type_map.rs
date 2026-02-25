// UE property type → Rust type / C++ FFI type mapping.

use crate::schema::ParamInfo;

/// Classification of a function parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// Input parameter.
    In,
    /// Output parameter (non-const out).
    Out,
    /// Input + output (reference param).
    InOut,
    /// Return value.
    Return,
}

/// Mapped type information for code generation.
#[derive(Debug, Clone)]
pub struct MappedType {
    /// Rust type in function signatures (e.g., "bool", "i32", "UObjectRef<AActor>").
    pub rust_type: String,
    /// Rust type for the FFI boundary (e.g., "bool", "i32", "UObjectHandle").
    pub rust_ffi_type: String,
    /// C++ type for the wrapper function (e.g., "bool", "int32", "UObject*").
    pub cpp_type: String,
    /// PropertyApi method name for getters (e.g., "get_bool", "get_i32").
    pub property_getter: String,
    /// PropertyApi method name for setters.
    pub property_setter: String,
    /// How to convert from Rust safe type to FFI type in function call.
    pub rust_to_ffi: ConversionKind,
    /// How to convert from FFI type to Rust safe type in return.
    pub ffi_to_rust: ConversionKind,
    /// Whether this is a supported type for Phase 3.
    pub supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionKind {
    /// No conversion needed (primitives).
    Identity,
    /// Integer type cast (e.g., u32 ↔ i32).
    IntCast,
    /// Wrap in UObjectRef::from_raw / extract with .raw().
    ObjectRef,
    /// String: UTF-8 ptr+len on FFI, String on Rust side.
    StringUtf8,
    /// Enum: i64 on FFI, enum type on Rust side.
    EnumCast,
    /// Struct: opaque pointer on FFI.
    StructOpaque,
    /// FName: FNameHandle on FFI.
    FName,
    /// TArray container property — returns UeArray<T> handle.
    ContainerArray,
    /// TMap container property — returns UeMap<K, V> handle.
    ContainerMap,
    /// TSet container property — returns UeSet<T> handle.
    ContainerSet,
    /// Unicast delegate property.
    Delegate,
    /// Multicast delegate property (inline or sparse).
    MulticastDelegate,
}

/// Supported UE property types.
const SUPPORTED_TYPES: &[&str] = &[
    "BoolProperty",
    "Int8Property",
    "ByteProperty",
    "Int16Property",
    "UInt16Property",
    "IntProperty",
    "UInt32Property",
    "Int64Property",
    "UInt64Property",
    "FloatProperty",
    "DoubleProperty",
    "StrProperty",
    "NameProperty",
    "TextProperty",
    "EnumProperty",
    "ObjectProperty",
    "ClassProperty",
    "StructProperty",
    "ArrayProperty",
    "MapProperty",
    "SetProperty",
    "SoftObjectProperty",
    "WeakObjectProperty",
    "InterfaceProperty",
    "DelegateProperty",
    "MulticastInlineDelegateProperty",
    "MulticastSparseDelegateProperty",
];

/// Check if a property type is supported in Phase 3.
pub fn is_supported_type(prop_type: &str) -> bool {
    SUPPORTED_TYPES.contains(&prop_type)
}

/// Map a UE property type string to its Rust/C++ type information.
pub fn map_property_type(
    prop_type: &str,
    class_name: Option<&str>,
    struct_name: Option<&str>,
    enum_name: Option<&str>,
    enum_underlying_type: Option<&str>,
    meta_class_name: Option<&str>,
    interface_name: Option<&str>,
) -> MappedType {
    match prop_type {
        "BoolProperty" => MappedType {
            rust_type: "bool".into(),
            rust_ffi_type: "bool".into(),
            cpp_type: "bool".into(),
            property_getter: "get_bool".into(),
            property_setter: "set_bool".into(),
            rust_to_ffi: ConversionKind::Identity,
            ffi_to_rust: ConversionKind::Identity,
            supported: true,
        },
        "Int8Property" => int_type("i8", "int8"),
        "ByteProperty" => {
            // ByteProperty can be a plain uint8 or an enum
            if let Some(en) = enum_name {
                enum_type(en, enum_underlying_type.unwrap_or("uint8"))
            } else {
                int_type("u8", "uint8")
            }
        }
        "Int16Property" => int_type("i16", "int16"),
        "UInt16Property" => int_type("u16", "uint16"),
        "IntProperty" => int_type("i32", "int32"),
        "UInt32Property" => int_type("u32", "uint32"),
        "Int64Property" => int_type("i64", "int64"),
        "UInt64Property" => int_type("u64", "uint64"),
        "FloatProperty" => MappedType {
            rust_type: "f32".into(),
            rust_ffi_type: "f32".into(),
            cpp_type: "float".into(),
            property_getter: "get_f32".into(),
            property_setter: "set_f32".into(),
            rust_to_ffi: ConversionKind::Identity,
            ffi_to_rust: ConversionKind::Identity,
            supported: true,
        },
        "DoubleProperty" => MappedType {
            rust_type: "f64".into(),
            rust_ffi_type: "f64".into(),
            cpp_type: "double".into(),
            property_getter: "get_f64".into(),
            property_setter: "set_f64".into(),
            rust_to_ffi: ConversionKind::Identity,
            ffi_to_rust: ConversionKind::Identity,
            supported: true,
        },
        "StrProperty" => MappedType {
            rust_type: "String".into(),
            rust_ffi_type: "*const u8".into(),
            cpp_type: "FString".into(),
            property_getter: "get_string".into(),
            property_setter: "set_string".into(),
            rust_to_ffi: ConversionKind::StringUtf8,
            ffi_to_rust: ConversionKind::StringUtf8,
            supported: true,
        },
        "TextProperty" => MappedType {
            rust_type: "String".into(),
            rust_ffi_type: "*const u8".into(),
            cpp_type: "FText".into(),
            property_getter: "get_string".into(),
            property_setter: "set_string".into(),
            rust_to_ffi: ConversionKind::StringUtf8,
            ffi_to_rust: ConversionKind::StringUtf8,
            supported: true,
        },
        "NameProperty" => MappedType {
            rust_type: "uika_runtime::FNameHandle".into(),
            rust_ffi_type: "uika_runtime::FNameHandle".into(),
            cpp_type: "FName".into(),
            property_getter: "get_fname".into(),
            property_setter: "set_fname".into(),
            rust_to_ffi: ConversionKind::FName,
            ffi_to_rust: ConversionKind::FName,
            supported: true,
        },
        "EnumProperty" => {
            if let Some(en) = enum_name {
                enum_type(en, enum_underlying_type.unwrap_or("uint8"))
            } else {
                unsupported("EnumProperty without enum_name")
            }
        }
        "ObjectProperty" => {
            if let Some(cls) = class_name {
                MappedType {
                    rust_type: format!("uika_runtime::UObjectRef<{cls}>"),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: format!("{cls}*"),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::ObjectRef,
                    ffi_to_rust: ConversionKind::ObjectRef,
                    supported: true,
                }
            } else {
                // Untyped object reference — use UObject
                MappedType {
                    rust_type: "uika_runtime::UObjectHandle".into(),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: "UObject*".into(),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::Identity,
                    ffi_to_rust: ConversionKind::Identity,
                    supported: true,
                }
            }
        }
        "SoftObjectProperty" | "WeakObjectProperty" => {
            // TSoftObjectPtr<T> / TWeakObjectPtr<T> resolve to UObject* via
            // FObjectPropertyBase — use the same ObjectRef mapping as ObjectProperty.
            if let Some(cls) = class_name {
                MappedType {
                    rust_type: format!("uika_runtime::UObjectRef<{cls}>"),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: format!("{cls}*"),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::ObjectRef,
                    ffi_to_rust: ConversionKind::ObjectRef,
                    supported: true,
                }
            } else {
                MappedType {
                    rust_type: "uika_runtime::UObjectHandle".into(),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: "UObject*".into(),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::Identity,
                    ffi_to_rust: ConversionKind::Identity,
                    supported: true,
                }
            }
        }
        "ClassProperty" => {
            let effective_class = meta_class_name.or(class_name);
            if let Some(cls) = effective_class {
                MappedType {
                    rust_type: format!("uika_runtime::UObjectRef<{cls}>"),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: format!("{cls}*"),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::ObjectRef,
                    ffi_to_rust: ConversionKind::ObjectRef,
                    supported: true,
                }
            } else {
                MappedType {
                    rust_type: "uika_runtime::UObjectHandle".into(),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: "UObject*".into(),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::Identity,
                    ffi_to_rust: ConversionKind::Identity,
                    supported: true,
                }
            }
        }
        "InterfaceProperty" => {
            if let Some(iface) = interface_name {
                MappedType {
                    rust_type: format!("uika_runtime::UObjectRef<{iface}>"),
                    rust_ffi_type: "uika_runtime::UObjectHandle".into(),
                    cpp_type: format!("{iface}*"),
                    property_getter: "get_object".into(),
                    property_setter: "set_object".into(),
                    rust_to_ffi: ConversionKind::ObjectRef,
                    ffi_to_rust: ConversionKind::ObjectRef,
                    supported: true,
                }
            } else {
                unsupported("InterfaceProperty without interface_name")
            }
        }
        "StructProperty" => {
            if let Some(sn) = struct_name {
                MappedType {
                    rust_type: format!("*const u8 /* {sn} */"),
                    rust_ffi_type: "*const u8".into(),
                    cpp_type: format!("F{sn}"),
                    property_getter: "get_struct".into(),
                    property_setter: "set_struct".into(),
                    rust_to_ffi: ConversionKind::StructOpaque,
                    ffi_to_rust: ConversionKind::StructOpaque,
                    supported: true,
                }
            } else {
                unsupported("StructProperty without struct_name")
            }
        }
        "ArrayProperty" => MappedType {
            rust_type: "uika_runtime::UeArray<_>".into(),
            rust_ffi_type: String::new(),
            cpp_type: String::new(),
            property_getter: String::new(),
            property_setter: String::new(),
            rust_to_ffi: ConversionKind::ContainerArray,
            ffi_to_rust: ConversionKind::ContainerArray,
            supported: true,
        },
        "MapProperty" => MappedType {
            rust_type: "uika_runtime::UeMap<_, _>".into(),
            rust_ffi_type: String::new(),
            cpp_type: String::new(),
            property_getter: String::new(),
            property_setter: String::new(),
            rust_to_ffi: ConversionKind::ContainerMap,
            ffi_to_rust: ConversionKind::ContainerMap,
            supported: true,
        },
        "SetProperty" => MappedType {
            rust_type: "uika_runtime::UeSet<_>".into(),
            rust_ffi_type: String::new(),
            cpp_type: String::new(),
            property_getter: String::new(),
            property_setter: String::new(),
            rust_to_ffi: ConversionKind::ContainerSet,
            ffi_to_rust: ConversionKind::ContainerSet,
            supported: true,
        },
        "DelegateProperty" => MappedType {
            rust_type: "/* delegate */".into(),
            rust_ffi_type: String::new(),
            cpp_type: String::new(),
            property_getter: String::new(),
            property_setter: String::new(),
            rust_to_ffi: ConversionKind::Delegate,
            ffi_to_rust: ConversionKind::Delegate,
            supported: true,
        },
        "MulticastInlineDelegateProperty" | "MulticastSparseDelegateProperty" => MappedType {
            rust_type: "/* multicast delegate */".into(),
            rust_ffi_type: String::new(),
            cpp_type: String::new(),
            property_getter: String::new(),
            property_setter: String::new(),
            rust_to_ffi: ConversionKind::MulticastDelegate,
            ffi_to_rust: ConversionKind::MulticastDelegate,
            supported: true,
        },
        _ => unsupported(prop_type),
    }
}

/// Map a param to its direction based on prop_flags.
pub fn param_direction(param: &ParamInfo) -> ParamDirection {
    use crate::schema::*;

    if param.prop_flags & CPF_RETURN_PARM != 0 {
        return ParamDirection::Return;
    }
    let is_out = param.prop_flags & CPF_OUT_PARM != 0;
    let is_const = param.prop_flags & CPF_CONST_PARM != 0;
    let is_ref = param.prop_flags & CPF_REFERENCE_PARM != 0;

    if is_out && is_const {
        // const& pseudo-output → actually input
        ParamDirection::In
    } else if is_out && is_ref {
        ParamDirection::InOut
    } else if is_out {
        ParamDirection::Out
    } else {
        ParamDirection::In
    }
}

fn int_type(rust: &str, _cpp: &str) -> MappedType {
    // Map to the FFI type that matches the available PropertyApi methods.
    // Available: get_u8/set_u8, get_i32/set_i32, get_i64/set_i64
    let (getter, setter, ffi_type) = match rust {
        "i8" => ("get_u8", "set_u8", "u8"),    // cast u8 ↔ i8
        "u8" => ("get_u8", "set_u8", "u8"),
        "i16" => ("get_i32", "set_i32", "i32"), // cast i32 ↔ i16
        "u16" => ("get_i32", "set_i32", "i32"), // cast i32 ↔ u16
        "i32" => ("get_i32", "set_i32", "i32"),
        "u32" => ("get_i32", "set_i32", "i32"), // cast i32 ↔ u32
        "i64" => ("get_i64", "set_i64", "i64"),
        "u64" => ("get_i64", "set_i64", "i64"), // cast i64 ↔ u64
        _ => ("get_i32", "set_i32", "i32"),
    };
    let cpp = match rust {
        "i8" => "int8",
        "u8" => "uint8",
        "i16" => "int16",
        "u16" => "uint16",
        "i32" => "int32",
        "u32" => "uint32",
        "i64" => "int64",
        "u64" => "uint64",
        _ => "int32",
    };
    let needs_cast = rust != ffi_type;
    MappedType {
        rust_type: rust.into(),
        rust_ffi_type: ffi_type.into(),
        cpp_type: cpp.into(),
        property_getter: getter.into(),
        property_setter: setter.into(),
        rust_to_ffi: if needs_cast { ConversionKind::IntCast } else { ConversionKind::Identity },
        ffi_to_rust: if needs_cast { ConversionKind::IntCast } else { ConversionKind::Identity },
        supported: true,
    }
}

fn enum_type(enum_name: &str, underlying: &str) -> MappedType {
    let repr = match underlying {
        "uint8" => "u8",
        "int8" => "i8",
        "uint16" => "u16",
        "int16" => "i16",
        "uint32" => "u32",
        "int32" => "i32",
        "uint64" => "u64",
        "int64" => "i64",
        _ => "u8",
    };
    MappedType {
        rust_type: enum_name.to_string(),
        rust_ffi_type: repr.into(),
        cpp_type: enum_name.to_string(),
        property_getter: "get_enum".into(),
        property_setter: "set_enum".into(),
        rust_to_ffi: ConversionKind::EnumCast,
        ffi_to_rust: ConversionKind::EnumCast,
        supported: true,
    }
}

fn unsupported(reason: &str) -> MappedType {
    MappedType {
        rust_type: format!("/* unsupported: {reason} */"),
        rust_ffi_type: String::new(),
        cpp_type: String::new(),
        property_getter: String::new(),
        property_setter: String::new(),
        rust_to_ffi: ConversionKind::Identity,
        ffi_to_rust: ConversionKind::Identity,
        supported: false,
    }
}

// ---------------------------------------------------------------------------
// Container inner-type resolution
// ---------------------------------------------------------------------------

use crate::context::CodegenContext;
use crate::schema::PropertyInfo;

/// Map an inner property (inside a container) to its Rust `ContainerElement` type.
/// When `ctx` is provided, validates that referenced types are in enabled modules
/// and returns the actual typed names (e.g., `UObjectRef<Actor>` instead of `UObjectHandle`).
/// Returns `None` if the inner type is unsupported for container elements.
pub fn container_element_rust_type(
    inner: &PropertyInfo,
    ctx: Option<&CodegenContext>,
) -> Option<String> {
    match inner.prop_type.as_str() {
        "BoolProperty" => Some("bool".into()),
        "Int8Property" => Some("i8".into()),
        "ByteProperty" => {
            if let Some(en) = &inner.enum_name {
                if let Some(ctx) = ctx {
                    if !ctx.enums.contains_key(en.as_str()) {
                        return None;
                    }
                }
                Some(en.clone())
            } else {
                Some("u8".into())
            }
        }
        "Int16Property" => Some("i16".into()),
        "UInt16Property" => Some("u16".into()),
        "IntProperty" => Some("i32".into()),
        "UInt32Property" => Some("u32".into()),
        "Int64Property" => Some("i64".into()),
        "UInt64Property" => Some("u64".into()),
        "FloatProperty" => Some("f32".into()),
        "DoubleProperty" => Some("f64".into()),
        "StrProperty" | "TextProperty" => Some("String".into()),
        "NameProperty" => Some("uika_runtime::FNameHandle".into()),
        "ObjectProperty" | "SoftObjectProperty" | "WeakObjectProperty" => {
            if let Some(cls) = &inner.class_name {
                if let Some(ctx) = ctx {
                    if !ctx.classes.contains_key(cls.as_str()) {
                        return None;
                    }
                }
                Some(format!("uika_runtime::UObjectRef<{cls}>"))
            } else {
                Some("uika_runtime::UObjectHandle".into())
            }
        }
        "ClassProperty" => {
            let effective_class = inner.meta_class_name.as_deref().or(inner.class_name.as_deref());
            if let Some(cls) = effective_class {
                if let Some(ctx) = ctx {
                    if !ctx.classes.contains_key(cls) {
                        return None;
                    }
                }
                Some(format!("uika_runtime::UObjectRef<{cls}>"))
            } else {
                Some("uika_runtime::UObjectHandle".into())
            }
        }
        "InterfaceProperty" => {
            if let Some(ref iface) = inner.interface_name {
                if let Some(ctx) = ctx {
                    if !ctx.classes.contains_key(iface.as_str()) {
                        return None;
                    }
                }
                Some(format!("uika_runtime::UObjectRef<{iface}>"))
            } else {
                None
            }
        }
        "EnumProperty" => {
            if let Some(en) = &inner.enum_name {
                if let Some(ctx) = ctx {
                    if !ctx.enums.contains_key(en.as_str()) {
                        return None;
                    }
                }
                Some(en.clone())
            } else {
                None
            }
        }
        "StructProperty" => {
            if let Some(sn) = &inner.struct_name {
                if let Some(ctx) = ctx {
                    if let Some(si) = ctx.structs.get(sn.as_str()) {
                        if si.has_static_struct {
                            Some(format!("uika_runtime::OwnedStruct<{}>", si.cpp_name))
                        } else {
                            None // No static_struct → no UeStruct impl
                        }
                    } else {
                        None // Not in enabled modules
                    }
                } else {
                    Some(format!("uika_runtime::OwnedStruct<F{sn}>"))
                }
            } else {
                None
            }
        }
        _ => None, // Nested containers etc. — unsupported
    }
}

/// Resolve the full Rust type for a container property.
/// Returns `None` if any inner type is unsupported.
pub fn resolve_container_rust_type(
    prop: &PropertyInfo,
    ctx: Option<&CodegenContext>,
) -> Option<String> {
    match prop.prop_type.as_str() {
        "ArrayProperty" => {
            let inner = prop.inner_prop.as_ref()?;
            let elem_type = container_element_rust_type(inner, ctx)?;
            Some(format!("uika_runtime::UeArray<{elem_type}>"))
        }
        "MapProperty" => {
            let key = prop.key_prop.as_ref()?;
            let val = prop.value_prop.as_ref()?;
            let key_type = container_element_rust_type(key, ctx)?;
            let val_type = container_element_rust_type(val, ctx)?;
            Some(format!("uika_runtime::UeMap<{key_type}, {val_type}>"))
        }
        "SetProperty" => {
            let elem = prop.element_prop.as_ref()?;
            let elem_type = container_element_rust_type(elem, ctx)?;
            Some(format!("uika_runtime::UeSet<{elem_type}>"))
        }
        _ => None,
    }
}
