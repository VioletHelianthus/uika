// C++ code generation orchestrator.

pub mod wrapper;
pub mod func_ids;
pub mod fill_table;

use std::collections::BTreeMap;
use std::path::Path;

use crate::context::CodegenContext;

/// Generate all C++ code into the output directory.
pub fn generate(ctx: &CodegenContext, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).expect("Failed to create C++ output directory");

    // Group func entries by (module, class) for per-file generation
    let mut by_class: BTreeMap<(String, String), Vec<&crate::context::FuncEntry>> = BTreeMap::new();
    for entry in &ctx.func_table {
        by_class
            .entry((entry.module_name.clone(), entry.class_name.clone()))
            .or_default()
            .push(entry);
    }

    // Generate per-class wrapper files
    for ((module, class), entries) in &by_class {
        let code = wrapper::generate_wrapper_file(entries, ctx);
        let filename = format!("UikaFunc_{}_{}.cpp", module, class);
        std::fs::write(out_dir.join(&filename), code)
            .unwrap_or_else(|e| panic!("Failed to write {filename}: {e}"));
    }

    // Generate UikaFuncIds.h
    let ids_code = func_ids::generate_cpp_func_ids(&ctx.func_table);
    std::fs::write(out_dir.join("UikaFuncIds.h"), ids_code)
        .expect("Failed to write UikaFuncIds.h");

    // Generate UikaFillFuncTable.cpp
    let fill_code = fill_table::generate_fill_table(&ctx.func_table, &by_class);
    std::fs::write(out_dir.join("UikaFillFuncTable.cpp"), fill_code)
        .expect("Failed to write UikaFillFuncTable.cpp");
}
