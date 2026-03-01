// Rust struct generation: opaque markers, UeStruct trait, and property accessors.

use crate::context::CodegenContext;
use crate::schema::StructInfo;

use super::properties::{self, PropertyContext};

/// Generate Rust code for a single UE struct.
pub fn generate_struct(s: &StructInfo, ctx: &CodegenContext) -> String {
    let mut out = String::with_capacity(2048);
    let name = &s.cpp_name; // Use cpp_name (e.g., "FVector") as the Rust type name
    let stripped = &s.name; // Stripped name (e.g., "Vector") for lookup

    // Import traits and types
    out.push_str("use super::*;\n");
    out.push_str("use uika_runtime::{UeClass, UeStruct, UeEnum};\n");
    let current_module = ctx
        .package_to_module
        .get(&s.package)
        .map(|s| s.as_str())
        .unwrap_or("");
    for module in &ctx.enabled_modules {
        if module != current_module {
            if let Some(feature) = ctx.feature_for_module(module) {
                out.push_str(&format!("#[cfg(feature = \"{feature}\")]\n"));
            }
            out.push_str(&format!("use crate::{module}::*;\n"));
        }
    }
    out.push('\n');

    out.push_str(&format!(
        "/// Opaque UE struct `{name}`. Layout managed by C++ side.\n\
         pub struct {name};\n\n"
    ));

    if s.has_static_struct {
        let name_bytes = stripped.as_bytes();
        let name_len = name_bytes.len();
        let byte_lit = format!("b\"{}\\0\"", stripped);

        out.push_str(&format!(
            "impl uika_runtime::UeStruct for {name} {{\n\
             \x20   fn static_struct() -> uika_runtime::UStructHandle {{\n\
             \x20       static CACHE: std::sync::OnceLock<uika_runtime::UStructHandle> = std::sync::OnceLock::new();\n\
             \x20       *CACHE.get_or_init(|| unsafe {{\n\
             \x20           uika_runtime::ffi_dispatch::reflection_find_struct({byte_lit}.as_ptr(), {name_len})\n\
             \x20       }})\n\
             \x20   }}\n\
             }}\n\n"
        ));

        // Generate property accessors if the struct has properties and static_struct
        let (_, deduped_props) = properties::collect_deduped_properties(&s.props, Some(ctx));
        if !deduped_props.is_empty() {
            let pctx = PropertyContext {
                find_prop_fn: "find_struct_property".to_string(),
                handle_expr: format!("{name}::static_struct()"),
                pre_access: String::new(), // No validity check for structs
                container_expr: "self.as_ptr()".to_string(),
                is_class: false,
            };

            let trait_name = format!("{name}Ext");

            // Generate full method bodies into a temp buffer
            let no_suppress = std::collections::HashSet::new();
            let mut body_buf = String::new();
            for prop in &deduped_props {
                properties::generate_property(&mut body_buf, prop, &pctx, ctx, &no_suppress);
            }

            // Trait declaration: extract signatures only (fn ... { → fn ...;)
            out.push_str(&format!("pub trait {trait_name} {{\n"));
            for line in body_buf.lines() {
                if line.starts_with("    fn ") && line.ends_with(" {") {
                    out.push_str(&line[..line.len() - 2]);
                    out.push_str(";\n");
                }
            }
            out.push_str("}\n\n");

            // Concrete impl on UStructRef<T> with full bodies
            out.push_str(&format!(
                "impl {trait_name} for uika_runtime::UStructRef<{name}> {{\n"
            ));
            out.push_str(&body_buf);
            out.push_str("}\n");
        }
    }

    out
}
