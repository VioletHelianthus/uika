// uika-codegen: reads UHT JSON, generates Rust bindings + C++ wrappers + FuncId tables.

pub mod schema;
pub mod naming;
pub mod config;
pub mod context;
pub mod type_map;
pub mod defaults;
pub mod filter;
pub mod rust_gen;
pub mod cpp_gen;

use std::path::Path;

use crate::config::UikaConfig;
use crate::schema::{ClassesFile, EnumsFile, StructsFile};

/// Run the generate command. Main entry point for codegen.
pub fn run_generate(config_path: &Path) {
    // Load config
    let config_str = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", config_path.display()));
    let uika_config: UikaConfig = toml::from_str(&config_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", config_path.display()));
    let codegen = &uika_config.codegen;

    // Resolve paths relative to config file directory
    let config_dir = config_path
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .unwrap_or_else(|e| panic!("Failed to canonicalize config dir: {e}"));

    // Derive JSON paths from config (relative to config dir)
    let uht_input = config_dir.join(&codegen.paths.uht_input);
    let classes_path = uht_input.join("uika_classes.json");
    let structs_path = uht_input.join("uika_structs.json");
    let enums_path = uht_input.join("uika_enums.json");
    let rust_out = config_dir.join(&codegen.paths.rust_out);
    let cpp_out = config_dir.join(&codegen.paths.cpp_out);

    eprintln!("uika-codegen: loading JSON...");

    // Parse JSON files
    let classes_json: ClassesFile = {
        let data = std::fs::read_to_string(&classes_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", classes_path.display()));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", classes_path.display()))
    };

    let structs_json: StructsFile = {
        let data = std::fs::read_to_string(&structs_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", structs_path.display()));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", structs_path.display()))
    };

    let enums_json: EnumsFile = {
        let data = std::fs::read_to_string(&enums_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", enums_path.display()));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", enums_path.display()))
    };

    eprintln!(
        "  Loaded {} classes, {} structs, {} enums",
        classes_json.classes.len(),
        structs_json.structs.len(),
        enums_json.enums.len()
    );

    // Build context
    let mut ctx = context::CodegenContext::new(
        classes_json.classes,
        structs_json.structs,
        enums_json.enums,
        codegen,
    );

    eprintln!(
        "  Enabled modules: {:?}",
        ctx.enabled_modules.iter().collect::<Vec<_>>()
    );
    for (module, classes) in &ctx.module_classes {
        eprintln!("    {}: {} classes", module, classes.len());
    }
    for (module, structs) in &ctx.module_structs {
        eprintln!("    {}: {} structs", module, structs.len());
    }
    for (module, enums) in &ctx.module_enums {
        eprintln!("    {}: {} enums", module, enums.len());
    }

    // Apply filters
    eprintln!("uika-codegen: filtering...");
    filter::apply_filters(&mut ctx, &codegen.blocklist);

    // Build function table (assign FuncIds)
    eprintln!("uika-codegen: building function table...");
    build_func_table(&mut ctx);
    eprintln!("  {} functions in func_table", ctx.func_table.len());

    // Generate Rust code
    eprintln!("uika-codegen: generating Rust code...");
    rust_gen::generate(&ctx, &rust_out);

    // Generate C++ code
    eprintln!("uika-codegen: generating C++ code...");
    cpp_gen::generate(&ctx, &cpp_out);

    // Generate module_deps.txt for Uika.Build.cs
    generate_module_deps(codegen, &cpp_out);

    // Post-generate verification
    eprintln!("uika-codegen: verifying output...");
    verify_output(&ctx, &rust_out, &cpp_out);

    eprintln!("uika-codegen: done!");
}

/// Generate module_deps.txt listing UE module names needed by enabled features.
fn generate_module_deps(config: &crate::config::CodegenConfig, cpp_out: &Path) {
    use std::collections::BTreeSet;

    let enabled_features: std::collections::HashSet<&str> =
        config.features.iter().map(|s| s.as_str()).collect();

    // Collect UE package names whose feature is enabled.
    // "Core" is always needed (UE base) and not in the modules map.
    let mut ue_modules: BTreeSet<&str> = BTreeSet::new();
    ue_modules.insert("Core");
    for (pkg, mapping) in &config.modules {
        if enabled_features.contains(mapping.feature.as_str()) {
            ue_modules.insert(pkg.as_str());
        }
    }

    let content = ue_modules
        .iter()
        .map(|m| *m)
        .collect::<Vec<_>>()
        .join("\n");

    let path = cpp_out.join("module_deps.txt");
    std::fs::write(&path, &content)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));

    eprintln!("  module_deps.txt: {:?}", ue_modules.iter().collect::<Vec<_>>());
}

/// Verify codegen output integrity.
fn verify_output(ctx: &context::CodegenContext, rust_out: &Path, cpp_out: &Path) {
    let mut errors: Vec<String> = Vec::new();

    // 1. FuncId contiguity: IDs must be 0..N-1 with no gaps
    for (i, entry) in ctx.func_table.iter().enumerate() {
        if entry.func_id != i as u32 {
            errors.push(format!(
                "FuncId gap: expected {} for module '{}' {}.{}, got {} (total functions: {})",
                i, entry.module_name, entry.class_name, entry.func_name,
                entry.func_id, ctx.func_table.len()
            ));
            break; // one error is enough to flag the issue
        }
    }

    // 2. Required Rust output files exist and are non-empty
    let rust_required = ["lib.rs", "func_ids.rs"];
    for name in &rust_required {
        let path = rust_out.join(name);
        match std::fs::metadata(&path) {
            Ok(m) if m.len() == 0 => errors.push(format!("Rust output empty: {}", path.display())),
            Err(_) => errors.push(format!("Rust output missing: {}", path.display())),
            _ => {}
        }
    }

    // 3. Per-module mod.rs exists for each enabled module
    for module_name in ctx.enabled_modules.iter() {
        let mod_path = rust_out.join(module_name).join("mod.rs");
        if !mod_path.exists() {
            errors.push(format!("Module mod.rs missing: {}", mod_path.display()));
        }
    }

    // 4. Required C++ output files exist and are non-empty
    let cpp_required = ["UikaFuncIds.h", "UikaFillFuncTable.cpp"];
    for name in &cpp_required {
        let path = cpp_out.join(name);
        match std::fs::metadata(&path) {
            Ok(m) if m.len() == 0 => errors.push(format!("C++ output empty: {}", path.display())),
            Err(_) => errors.push(format!("C++ output missing: {}", path.display())),
            _ => {}
        }
    }

    // 5. Summary stats
    let func_count = ctx.func_table.len();
    let module_count = ctx.enabled_modules.len();
    let class_count: usize = ctx.module_classes.values().map(|v| v.len()).sum();
    let struct_count: usize = ctx.module_structs.values().map(|v| v.len()).sum();
    let enum_count: usize = ctx.module_enums.values().map(|v| v.len()).sum();

    if errors.is_empty() {
        eprintln!(
            "  OK: {} modules, {} classes, {} structs, {} enums, {} functions",
            module_count, class_count, struct_count, enum_count, func_count
        );
    } else {
        eprintln!("  Verification FAILED:");
        for e in &errors {
            eprintln!("    - {e}");
        }
        std::process::exit(1);
    }
}

/// Assign deterministic FuncIds to all exportable functions.
fn build_func_table(ctx: &mut context::CodegenContext) {
    let mut entries = Vec::new();

    for (module_name, classes) in &ctx.module_classes {
        for class in classes {
            // Skip UInterface classes — their functions can't be called directly
            if class.super_class.as_deref() == Some("Interface") {
                continue;
            }
            for func in &class.funcs {
                // Skip functions with unsupported param types
                let all_supported = func.params.iter().all(|p| {
                    type_map::map_property_type(
                        &p.prop_type,
                        p.class_name.as_deref(),
                        p.struct_name.as_deref(),
                        p.enum_name.as_deref(),
                        p.enum_underlying_type.as_deref(),
                        p.meta_class_name.as_deref(),
                        p.interface_name.as_deref(),
                    )
                    .supported
                });
                if !all_supported {
                    continue;
                }
                entries.push(context::FuncEntry {
                    func_id: 0, // assigned below
                    module_name: module_name.clone(),
                    class_name: class.name.clone(),
                    func_name: func.name.clone(),
                    rust_func_name: naming::to_snake_case(&func.name),
                    func: func.clone(),
                    cpp_class_name: class.cpp_name.clone(),
                    header: class.header.clone(),
                });
            }
        }
    }

    // Sort by (module, class, func) for deterministic IDs
    entries.sort_by(|a, b| {
        a.module_name
            .cmp(&b.module_name)
            .then_with(|| a.class_name.cmp(&b.class_name))
            .then_with(|| a.func_name.cmp(&b.func_name))
    });

    // Assign sequential IDs
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.func_id = i as u32;
    }

    ctx.func_table = entries;
}
