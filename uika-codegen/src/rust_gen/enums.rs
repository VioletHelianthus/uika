// Rust enum generation from UHT JSON.

use crate::naming::escape_reserved;
use crate::schema::EnumInfo;

/// Generate Rust code for a single UE enum.
pub fn generate_enum(e: &EnumInfo) -> String {
    let mut out = String::with_capacity(2048);

    let name = &e.name;

    // Filter out MAX/Count sentinel values and duplicates
    let mut seen_values = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    let mut variants: Vec<(&str, i64)> = Vec::new();
    for (variant_name, value) in &e.pairs {
        // Skip MAX sentinel and _MAX patterns
        if variant_name.ends_with("_MAX")
            || variant_name == "MAX"
            || variant_name.contains("__MAX")
        {
            continue;
        }
        // Skip duplicate values
        if !seen_values.insert(*value) {
            continue;
        }
        // Skip duplicate names (after prefix stripping)
        let clean = strip_enum_prefix(variant_name, &e.name);
        let sanitized = sanitize_variant_name(clean);
        if !seen_names.insert(sanitized) {
            continue;
        }
        variants.push((variant_name, *value));
    }

    if variants.is_empty() {
        // Empty enum — generate as a newtype wrapper
        let repr = underlying_to_repr(&e.underlying_type);
        out.push_str(&format!(
            "// Empty enum {name}\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
             pub struct {name}(pub {repr});\n"
        ));
        out.push_str(&format!(
            "\nimpl uika_runtime::UeEnum for {name} {{\n    type Repr = {repr};\n}}\n"
        ));
        generate_newtype_container_element(&mut out, name, repr);
        return out;
    }

    // Determine repr: if there are negative values and the UE type is unsigned,
    // promote to the signed equivalent. Also normalize values to avoid collisions
    // (e.g., 255u8 == -1i8).
    let has_negative = variants.iter().any(|(_, v)| *v < 0);
    let repr = if has_negative {
        underlying_to_signed_repr(&e.underlying_type)
    } else {
        underlying_to_repr(&e.underlying_type)
    };

    // Normalize values through the repr type to detect actual duplicates
    if has_negative {
        let mut seen_normalized = std::collections::HashSet::new();
        variants.retain(|(_, v)| {
            let normalized = normalize_to_signed(*v, &e.underlying_type);
            seen_normalized.insert(normalized)
        });
    }

    // Enum definition
    out.push_str(&format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         #[repr({repr})]\n\
         pub enum {name} {{\n"
    ));

    for (variant_name, value) in &variants {
        let clean_name = strip_enum_prefix(variant_name, name);
        let safe_name = escape_reserved(&sanitize_variant_name(clean_name));
        let display_value = if has_negative {
            normalize_to_signed(*value, &e.underlying_type)
        } else {
            *value
        };
        out.push_str(&format!("    {safe_name} = {display_value},\n"));
    }

    out.push_str("}\n");

    // from_value method
    out.push_str(&format!(
        "\nimpl {name} {{\n    pub fn from_value(v: {repr}) -> Option<Self> {{\n        match v {{\n"
    ));
    for (variant_name, value) in &variants {
        let clean_name = strip_enum_prefix(variant_name, name);
        let safe_name = escape_reserved(&sanitize_variant_name(clean_name));
        let display_value = if has_negative {
            normalize_to_signed(*value, &e.underlying_type)
        } else {
            *value
        };
        out.push_str(&format!("            {display_value} => Some({name}::{safe_name}),\n"));
    }
    out.push_str("            _ => None,\n        }\n    }\n\n");

    // display_name method
    out.push_str("    pub fn display_name(&self) -> &'static str {\n        match self {\n");
    for (variant_name, _) in &variants {
        let clean_name = strip_enum_prefix(variant_name, name);
        let safe_name = escape_reserved(&sanitize_variant_name(clean_name));
        out.push_str(&format!(
            "            {name}::{safe_name} => \"{clean_name}\",\n"
        ));
    }
    out.push_str("        }\n    }\n}\n");

    // UeEnum impl
    out.push_str(&format!(
        "\nimpl uika_runtime::UeEnum for {name} {{\n    type Repr = {repr};\n}}\n"
    ));

    // ContainerElement impl — allows this enum to be used as TArray/TMap/TSet element
    generate_enum_container_element(&mut out, name, repr);

    out
}

/// Map UE underlying type string to Rust repr type.
fn underlying_to_repr(ut: &str) -> &'static str {
    match ut {
        "uint8" => "u8",
        "int8" => "i8",
        "uint16" => "u16",
        "int16" => "i16",
        "uint32" => "u32",
        "int32" => "i32",
        "uint64" => "u64",
        "int64" => "i64",
        _ => "u8",
    }
}

/// Map unsigned UE type to signed equivalent (for enums with negative values).
fn underlying_to_signed_repr(ut: &str) -> &'static str {
    match ut {
        "uint8" => "i8",
        "int8" => "i8",
        "uint16" => "i16",
        "int16" => "i16",
        "uint32" => "i32",
        "int32" => "i32",
        "uint64" => "i64",
        "int64" => "i64",
        _ => "i8",
    }
}

/// Strip enum name prefix from variant names (e.g., "EFoo::Bar" → "Bar",
/// or if variants have EnumName_ prefix).
fn strip_enum_prefix<'a>(variant: &'a str, enum_name: &str) -> &'a str {
    if let Some(rest) = variant.strip_prefix(enum_name) {
        if let Some(rest) = rest.strip_prefix("::") {
            return rest;
        }
    }
    variant
}

/// Normalize a value to signed representation through the type's width.
/// e.g., 255u8 → -1i8, but 127u8 → 127i8.
fn normalize_to_signed(value: i64, underlying_type: &str) -> i64 {
    match underlying_type {
        "uint8" => (value as u8) as i8 as i64,
        "int8" => (value as i8) as i64,
        "uint16" => (value as u16) as i16 as i64,
        "int16" => (value as i16) as i64,
        "uint32" => (value as u32) as i32 as i64,
        "int32" => (value as i32) as i64,
        _ => value,
    }
}

/// Generate `ContainerElement` impl for a `#[repr(X)]` enum.
fn generate_enum_container_element(out: &mut String, name: &str, repr: &str) {
    out.push_str(&format!(
        "\nunsafe impl uika_runtime::ContainerElement for {name} {{\n\
         \x20   const BUF_SIZE: u32 = std::mem::size_of::<{repr}>() as u32;\n\n\
         \x20   #[inline]\n\
         \x20   unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self {{\n\
         \x20       std::mem::transmute::<{repr}, Self>((buf as *const {repr}).read_unaligned())\n\
         \x20   }}\n\n\
         \x20   #[inline]\n\
         \x20   unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 {{\n\
         \x20       let raw: {repr} = std::mem::transmute(*self);\n\
         \x20       (buf as *mut {repr}).write_unaligned(raw);\n\
         \x20       std::mem::size_of::<{repr}>() as u32\n\
         \x20   }}\n\
         }}\n"
    ));
}

/// Generate `ContainerElement` impl for a newtype-wrapper empty enum.
fn generate_newtype_container_element(out: &mut String, name: &str, repr: &str) {
    out.push_str(&format!(
        "\nunsafe impl uika_runtime::ContainerElement for {name} {{\n\
         \x20   const BUF_SIZE: u32 = std::mem::size_of::<{repr}>() as u32;\n\n\
         \x20   #[inline]\n\
         \x20   unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self {{\n\
         \x20       Self((buf as *const {repr}).read_unaligned())\n\
         \x20   }}\n\n\
         \x20   #[inline]\n\
         \x20   unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 {{\n\
         \x20       (buf as *mut {repr}).write_unaligned(self.0);\n\
         \x20       std::mem::size_of::<{repr}>() as u32\n\
         \x20   }}\n\
         }}\n"
    ));
}

/// Sanitize variant name to be a valid Rust identifier.
fn sanitize_variant_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for (i, ch) in name.chars().enumerate() {
        if ch.is_alphanumeric() || ch == '_' {
            if i == 0 && ch.is_ascii_digit() {
                result.push('_');
            }
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        result.push_str("_Unknown");
    }
    result
}
