// Rust code generation orchestrator.

pub mod enums;
pub mod structs;
pub mod classes;
pub mod properties;
pub mod delegates;
pub mod module;
pub mod func_ids;
pub mod wasm_gen;
pub mod param_helpers;

use std::path::Path;

use crate::context::CodegenContext;

/// Generate all Rust code into the output directory.
pub fn generate(ctx: &CodegenContext, out_dir: &Path) {
    // Ensure output directory exists
    std::fs::create_dir_all(out_dir).expect("Failed to create Rust output directory");

    // Generate per-module code
    for module_name in ctx.enabled_modules.iter() {
        let module_dir = out_dir.join(module_name);
        std::fs::create_dir_all(&module_dir).expect("Failed to create module directory");

        // Enums
        if let Some(module_enums) = ctx.module_enums.get(module_name) {
            for e in module_enums {
                let code = enums::generate_enum(e);
                let filename = crate::naming::to_snake_case(&e.name) + ".rs";
                std::fs::write(module_dir.join(&filename), code)
                    .unwrap_or_else(|err| panic!("Failed to write {filename}: {err}"));
            }
        }

        // Structs
        if let Some(module_structs) = ctx.module_structs.get(module_name) {
            for s in module_structs {
                let code = structs::generate_struct(s, ctx);
                let filename = crate::naming::to_snake_case(&s.name) + ".rs";
                std::fs::write(module_dir.join(&filename), code)
                    .unwrap_or_else(|err| panic!("Failed to write {filename}: {err}"));
            }
        }

        // Classes
        if let Some(module_classes) = ctx.module_classes.get(module_name) {
            for c in module_classes {
                let code = classes::generate_class(c, ctx);
                let filename = crate::naming::to_snake_case(&c.name) + ".rs";
                std::fs::write(module_dir.join(&filename), code)
                    .unwrap_or_else(|err| panic!("Failed to write {filename}: {err}"));
            }
        }

        // Module mod.rs
        let mod_code = module::generate_module_mod(
            module_name,
            ctx.module_enums.get(module_name).map(|v| v.as_slice()),
            ctx.module_structs.get(module_name).map(|v| v.as_slice()),
            ctx.module_classes.get(module_name).map(|v| v.as_slice()),
        );
        std::fs::write(module_dir.join("mod.rs"), mod_code)
            .expect("Failed to write module mod.rs");
    }

    // Generate func_ids.rs
    let func_ids_code = func_ids::generate_rust_func_ids(&ctx.func_table);
    std::fs::write(out_dir.join("func_ids.rs"), func_ids_code)
        .expect("Failed to write func_ids.rs");

    // Generate wasm_fn_imports.rs (WASM extern imports for codegen functions)
    let wasm_imports_code = wasm_gen::generate_wasm_fn_imports(&ctx.func_table, ctx);
    std::fs::write(out_dir.join("wasm_fn_imports.rs"), wasm_imports_code)
        .expect("Failed to write wasm_fn_imports.rs");

    // Generate wasm_host_codegen_funcs.rs (host function registration)
    // Output to uika-wasm-host/src/generated/ (relative to uika-bindings/src/)
    let wasm_host_dir = out_dir
        .parent()  // uika-bindings/
        .and_then(|p| p.parent())  // uika/
        .map(|p| p.join("uika-wasm-host").join("src").join("generated"));
    if let Some(wasm_host_dir) = wasm_host_dir {
        if wasm_host_dir.parent().map_or(false, |p| p.exists()) {
            std::fs::create_dir_all(&wasm_host_dir).ok();
            let host_funcs_code = wasm_gen::generate_wasm_host_funcs(&ctx.func_table, ctx);
            std::fs::write(wasm_host_dir.join("codegen_host_funcs.rs"), host_funcs_code)
                .expect("Failed to write codegen_host_funcs.rs");
        }
    }

    // Generate top-level lib.rs
    let lib_code = module::generate_lib_rs(ctx);
    std::fs::write(out_dir.join("lib.rs"), lib_code).expect("Failed to write lib.rs");
}
