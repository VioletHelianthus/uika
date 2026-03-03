// Secondary filtering: K2_ dedup, FUNC_Native gate, type exportability, overloads.

use std::collections::{HashMap, HashSet};

use crate::config::Blocklist;
use crate::context::CodegenContext;
use crate::schema::*;
use crate::type_map;

/// Apply all filters to the context's module_classes in place.
pub fn apply_filters(ctx: &mut CodegenContext, blocklist: &Blocklist) {
    // Pre-collect the set of available types to avoid borrowing ctx inside the loop.
    let available_types: HashSet<String> = ctx
        .classes
        .keys()
        .chain(ctx.structs.keys())
        .chain(ctx.enums.keys())
        .cloned()
        .collect();

    // Build lookup sets from config blocklist
    let blocked_classes: HashSet<&str> = blocklist.classes.iter().map(|s| s.as_str()).collect();
    let blocked_structs: HashSet<&str> = blocklist.structs.iter().map(|s| s.as_str()).collect();
    let blocked_functions: Vec<(String, String)> = blocklist.function_tuples();

    // Remove blocked classes from both module_classes and ctx.classes
    for cls in &blocklist.classes {
        ctx.classes.remove(cls);
    }

    for classes in ctx.module_classes.values_mut() {
        // Remove blocked classes entirely
        classes.retain(|c| !blocked_classes.contains(c.name.as_str()));

        for class in classes.iter_mut() {
            // Filter properties
            class
                .props
                .retain(|p| is_property_exportable(p, &available_types));

            // Filter functions
            filter_functions(&class.name, &mut class.funcs, &available_types, &blocked_structs, &blocked_functions);
        }
    }
}

/// Check if a property is exportable (supported type, not private/protected, single array dim).
fn is_property_exportable(prop: &PropertyInfo, available: &HashSet<String>) -> bool {
    // Skip unsupported types
    if !type_map::is_supported_type(&prop.prop_type) {
        return false;
    }

    // Skip fixed arrays of string/name/text types (CopySingleValue not safe for FString)
    if prop.array_dim > 1 {
        match prop.prop_type.as_str() {
            "StrProperty" | "NameProperty" | "TextProperty" => return false,
            _ => {} // allow through
        }
    }

    // Skip private/protected
    if prop.prop_flags & CPF_NATIVE_ACCESS_PRIVATE != 0 {
        return false;
    }
    if prop.prop_flags & CPF_NATIVE_ACCESS_PROTECTED != 0 {
        return false;
    }

    // Delegate properties: validate all params in func_info are exportable
    if is_delegate_type(&prop.prop_type) {
        return is_delegate_exportable(prop, available);
    }

    // Check referenced types are available
    if let Some(ref cls) = prop.class_name {
        if !available.contains(cls) {
            return false;
        }
    }
    if let Some(ref sn) = prop.struct_name {
        if !available.contains(sn) {
            return false;
        }
    }
    if let Some(ref en) = prop.enum_name {
        if !available.contains(en) {
            return false;
        }
    }
    if let Some(ref iface) = prop.interface_name {
        if !available.contains(iface) {
            return false;
        }
    }
    if prop.prop_type == "ClassProperty" {
        if let Some(ref meta_cls) = prop.meta_class_name {
            if !available.contains(meta_cls) {
                return false;
            }
        }
    }

    true
}

fn is_delegate_type(prop_type: &str) -> bool {
    matches!(
        prop_type,
        "DelegateProperty" | "MulticastInlineDelegateProperty" | "MulticastSparseDelegateProperty"
    )
}

/// Check if a delegate property's func_info params are all exportable.
fn is_delegate_exportable(prop: &PropertyInfo, available: &HashSet<String>) -> bool {
    let func_info = match &prop.func_info {
        Some(fi) => fi,
        None => return false, // No signature info — can't export
    };

    // Parse func_info params
    let params = match func_info.get("params").and_then(|p| p.as_array()) {
        Some(params) => params,
        None => return true, // No params — zero-arg delegate, always exportable
    };

    for param_value in params {
        let param_type = match param_value.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => return false,
        };

        // Each delegate param type must be supported
        if !type_map::is_supported_type(param_type) {
            return false;
        }

        // Delegate params cannot themselves be delegates or containers
        if is_delegate_type(param_type) {
            return false;
        }
        if matches!(param_type, "ArrayProperty" | "MapProperty" | "SetProperty") {
            return false;
        }

        // Check referenced types are available
        if let Some(cls) = param_value.get("class_name").and_then(|v| v.as_str()) {
            if !available.contains(cls) {
                return false;
            }
        }
        if let Some(sn) = param_value.get("struct_name").and_then(|v| v.as_str()) {
            if !available.contains(sn) {
                return false;
            }
        }
        if let Some(en) = param_value.get("enum_name").and_then(|v| v.as_str()) {
            if !available.contains(en) {
                return false;
            }
        }
    }

    true
}

/// Check if a container parameter's inner types are exportable.
fn is_container_param_exportable(param: &ParamInfo, available: &HashSet<String>) -> bool {
    match param.prop_type.as_str() {
        "ArrayProperty" => {
            if let Some(ref inner) = param.inner_prop {
                is_inner_type_exportable(inner, available)
            } else {
                false
            }
        }
        "MapProperty" => {
            let key_ok = param.key_prop.as_ref()
                .map(|k| is_inner_type_exportable(k, available))
                .unwrap_or(false);
            let val_ok = param.value_prop.as_ref()
                .map(|v| is_inner_type_exportable(v, available))
                .unwrap_or(false);
            key_ok && val_ok
        }
        "SetProperty" => {
            if let Some(ref elem) = param.element_prop {
                is_inner_type_exportable(elem, available)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Check if a container inner type is supported and its referenced types are available.
fn is_inner_type_exportable(inner: &PropertyInfo, available: &HashSet<String>) -> bool {
    if !type_map::is_supported_type(&inner.prop_type) {
        return false;
    }
    // Nested containers not supported
    if matches!(inner.prop_type.as_str(), "ArrayProperty" | "MapProperty" | "SetProperty") {
        return false;
    }
    if let Some(ref cls) = inner.class_name {
        if !available.contains(cls) {
            return false;
        }
    }
    if let Some(ref sn) = inner.struct_name {
        if !available.contains(sn) {
            return false;
        }
    }
    if let Some(ref en) = inner.enum_name {
        if !available.contains(en) {
            return false;
        }
    }
    if let Some(ref iface) = inner.interface_name {
        if !available.contains(iface) {
            return false;
        }
    }
    true
}

/// Filter functions on a class: FUNC_Native gate, K2_ dedup, param type check, overload rename.
fn filter_functions(
    class_name: &str,
    funcs: &mut Vec<FunctionInfo>,
    available: &HashSet<String>,
    blocked_structs: &HashSet<&str>,
    blocked_functions: &[(String, String)],
) {
    // Step 1: Collect all function names for K2_ dedup
    let all_names: HashSet<String> = funcs.iter().map(|f| f.name.clone()).collect();

    // Step 2: Filter
    funcs.retain(|f| {
        // Function-level blocklist (unlinked symbols)
        if blocked_functions.iter().any(|(c, func)| c == class_name && func == &f.name) {
            return false;
        }

        // FUNC_Native gate
        if f.func_flags & FUNC_NATIVE == 0 {
            return false;
        }

        // K2_ dedup: if this is K2_Foo and Foo also exists, skip K2_Foo
        if f.name.starts_with("K2_") {
            let base_name = &f.name[3..];
            if all_names.contains(base_name) {
                return false;
            }
        }

        // Check all param types are supported and referenced types are available
        for param in &f.params {
            if !type_map::is_supported_type(&param.prop_type) {
                return false;
            }
            // Delegate-typed params are not valid in function signatures
            if is_delegate_type(&param.prop_type) {
                return false;
            }
            // Check container inner types are resolvable
            if matches!(param.prop_type.as_str(), "ArrayProperty" | "MapProperty" | "SetProperty") {
                if !is_container_param_exportable(param, available) {
                    return false;
                }
            }
            if let Some(ref cls) = param.class_name {
                if !available.contains(cls) {
                    return false;
                }
            }
            if let Some(ref sn) = param.struct_name {
                if !available.contains(sn) || blocked_structs.contains(sn.as_str()) {
                    return false;
                }
            }
            if let Some(ref en) = param.enum_name {
                if !available.contains(en) {
                    return false;
                }
            }
            if let Some(ref iface) = param.interface_name {
                if !available.contains(iface) {
                    return false;
                }
            }
            if param.prop_type == "ClassProperty" {
                if let Some(ref meta_cls) = param.meta_class_name {
                    if !available.contains(meta_cls) {
                        return false;
                    }
                }
            }
        }

        true
    });

    // Preserve original UE function names before overload renaming
    for f in funcs.iter_mut() {
        f.ue_name = f.name.clone();
    }

    // Step 3: Handle overloads — rename duplicates with _1, _2 suffix
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for f in funcs.iter() {
        *name_counts.entry(f.name.clone()).or_default() += 1;
    }

    let mut name_indices: HashMap<String, usize> = HashMap::new();
    for f in funcs.iter_mut() {
        if let Some(&count) = name_counts.get(&f.name) {
            if count > 1 {
                let idx = name_indices.entry(f.name.clone()).or_insert(0);
                *idx += 1;
                f.name = format!("{}_{}", f.name, idx);
            }
        }
    }
}
