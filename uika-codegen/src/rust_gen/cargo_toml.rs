// Regenerates the `[features]` section of `uika-bindings/Cargo.toml`
// from the codegen module dependency graph.
//
// All other sections of the file are preserved verbatim. If the file does
// not yet contain a `[features]` section, one is appended.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::config::CodegenConfig;
use crate::context::CodegenContext;

/// Update the `[features]` table in `uika-bindings/Cargo.toml`.
///
/// `cargo_toml_path` should point at the existing Cargo.toml. Sections
/// before `[features]` (e.g. `[package]`, `[dependencies]`) are kept
/// untouched.
pub fn write_features_section(
    cargo_toml_path: &Path,
    ctx: &CodegenContext,
    config: &CodegenConfig,
) {
    let existing = std::fs::read_to_string(cargo_toml_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read {} for [features] update: {e}",
            cargo_toml_path.display()
        )
    });

    let default_value = parse_default_feature(&existing);
    let existing_features = parse_existing_features(&existing);
    let new_section = render_features_section(
        ctx,
        config,
        default_value.as_deref(),
        &existing_features,
    );
    let updated = replace_features_section(&existing, &new_section);

    std::fs::write(cargo_toml_path, updated).unwrap_or_else(|e| {
        panic!(
            "Failed to write updated {}: {e}",
            cargo_toml_path.display()
        )
    });

    eprintln!(
        "  uika-bindings/Cargo.toml [features] rewritten ({} features)",
        config.modules.len() + 1
    );
}

/// Render a complete `[features]` table starting with `[features]\n`.
///
/// For features whose module is enabled in this codegen run, deps are
/// computed from the cross-module reference graph. For features whose
/// module is not enabled, existing deps from `existing_features` are
/// preserved verbatim — re-running codegen with that feature enabled
/// will then refresh them with real data.
fn render_features_section(
    ctx: &CodegenContext,
    config: &CodegenConfig,
    default_value: Option<&str>,
    existing_features: &BTreeMap<String, Vec<String>>,
) -> String {
    let mut all_features: BTreeSet<String> = BTreeSet::new();
    for mapping in config.modules.values() {
        all_features.insert(mapping.feature.clone());
    }
    for k in existing_features.keys() {
        if k != "default" {
            all_features.insert(k.clone());
        }
    }

    // Reverse map: feature name → modules. Some features (e.g. "slate")
    // map from multiple modules (SlateCore + Slate); we union their deps.
    let mut feature_to_modules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for mapping in config.modules.values() {
        feature_to_modules
            .entry(mapping.feature.clone())
            .or_default()
            .push(mapping.module.clone());
    }

    let enabled_features: BTreeSet<&str> =
        config.features.iter().map(|s| s.as_str()).collect();

    let mut out = String::new();
    out.push_str("[features]\n");

    let default_deps = default_value.unwrap_or("[\"core\"]");
    out.push_str(&format!("default = {default_deps}\n"));

    for feature in &all_features {
        let deps_array = if enabled_features.contains(feature.as_str()) {
            // Compute from data.
            let mut dep_features: BTreeSet<String> = BTreeSet::new();
            if let Some(modules) = feature_to_modules.get(feature) {
                for module in modules {
                    if let Some(module_deps) = ctx.module_deps.get(module) {
                        for dep_module in module_deps {
                            if let Some(dep_feature) = ctx.feature_for_module(dep_module) {
                                if dep_feature != feature {
                                    dep_features.insert(dep_feature.to_string());
                                }
                            }
                        }
                    }
                }
            }
            format_dep_array(&dep_features.iter().cloned().collect::<Vec<_>>())
        } else if let Some(existing_deps) = existing_features.get(feature) {
            // Preserve existing deps for features not enabled this run.
            format_dep_array(existing_deps)
        } else {
            "[]".to_string()
        };
        out.push_str(&format!("{feature} = {deps_array}\n"));
    }

    out
}

fn format_dep_array(deps: &[String]) -> String {
    if deps.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = deps.iter().map(|d| format!("\"{d}\"")).collect();
        format!("[{}]", items.join(", "))
    }
}

/// Parse all feature entries (besides `default`) from the existing Cargo.toml.
fn parse_existing_features(existing: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let value: toml::Value = match toml::from_str(existing) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let Some(features) = value.get("features").and_then(|f| f.as_table()) else {
        return out;
    };
    for (k, v) in features {
        if k == "default" {
            continue;
        }
        let Some(arr) = v.as_array() else { continue };
        let deps: Vec<String> = arr
            .iter()
            .filter_map(|d| d.as_str().map(|s| s.to_string()))
            .collect();
        out.insert(k.clone(), deps);
    }
    out
}

/// Replace the `[features]` block in `existing` with `new_section`.
///
/// `new_section` must include the `[features]` header line. The block spans
/// from `[features]` to the next `[section]` header or EOF.
fn replace_features_section(existing: &str, new_section: &str) -> String {
    let lines: Vec<&str> = existing.lines().collect();
    let mut start: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("[features]") {
            start = Some(i);
            break;
        }
    }

    let trailing_newline = existing.ends_with('\n');

    let mut new_section_owned = new_section.to_string();
    if !new_section_owned.ends_with('\n') {
        new_section_owned.push('\n');
    }

    match start {
        None => {
            // Append a fresh [features] block.
            let mut out = existing.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&new_section_owned);
            out
        }
        Some(start_idx) => {
            // Find the next section header after [features].
            let mut end_idx = lines.len();
            for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
                let trimmed = line.trim_start();
                if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
                    end_idx = i;
                    break;
                }
            }

            // Trim blank lines immediately before the next section so we
            // don't accumulate them on repeated runs.
            let mut effective_end = end_idx;
            while effective_end > start_idx + 1 && lines[effective_end - 1].trim().is_empty() {
                effective_end -= 1;
            }

            let mut out = String::new();
            for line in &lines[..start_idx] {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&new_section_owned);
            if effective_end < lines.len() {
                out.push('\n');
                for line in &lines[effective_end..] {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            if !trailing_newline && out.ends_with('\n') {
                out.pop();
            }
            out
        }
    }
}

/// Parse `default = [...]` from the existing `[features]` table.
/// Returns the value formatted as an inline TOML array (e.g. `["core"]`).
fn parse_default_feature(existing: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(existing).ok()?;
    let features = value.get("features")?.as_table()?;
    let default = features.get("default")?.as_array()?;
    let items: Vec<String> = default
        .iter()
        .filter_map(|v| v.as_str().map(|s| format!("\"{s}\"")))
        .collect();
    Some(format!("[{}]", items.join(", ")))
}
