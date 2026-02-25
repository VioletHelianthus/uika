// Build context: type lookups, module mapping, FuncId assignment.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::config::CodegenConfig;
use crate::schema::{ClassInfo, EnumInfo, FunctionInfo, StructInfo};

/// Central build context for the codegen pipeline.
pub struct CodegenContext {
    /// All classes by name.
    pub classes: HashMap<String, ClassInfo>,
    /// All structs by name.
    pub structs: HashMap<String, StructInfo>,
    /// All enums by name.
    pub enums: HashMap<String, EnumInfo>,

    /// Package name → Rust module name.
    pub package_to_module: HashMap<String, String>,
    /// Rust module name → Cargo feature name.
    pub module_to_feature: HashMap<String, String>,
    /// Set of enabled Rust module names (derived from features).
    pub enabled_modules: HashSet<String>,

    /// Classes grouped by module name. Sorted for determinism.
    pub module_classes: BTreeMap<String, Vec<ClassInfo>>,
    /// Structs grouped by module name.
    pub module_structs: BTreeMap<String, Vec<StructInfo>>,
    /// Enums grouped by module name.
    pub module_enums: BTreeMap<String, Vec<EnumInfo>>,

    /// All exportable functions sorted by (module, class, func) for FuncId assignment.
    pub func_table: Vec<FuncEntry>,
}

/// An entry in the global function table.
#[derive(Clone)]
pub struct FuncEntry {
    pub func_id: u32,
    pub module_name: String,
    pub class_name: String,
    pub func_name: String,
    /// The actual resolved Rust function name (may include _1, _2 suffix for overloads).
    pub rust_func_name: String,
    pub func: FunctionInfo,
    /// The C++ class name (with prefix, e.g. "AActor").
    pub cpp_class_name: String,
    /// The header file to include.
    pub header: String,
}

impl CodegenContext {
    pub fn new(
        classes: Vec<ClassInfo>,
        structs: Vec<StructInfo>,
        enums: Vec<EnumInfo>,
        config: &CodegenConfig,
    ) -> Self {
        // Build package→module and module→feature maps from config
        let mut package_to_module = HashMap::new();
        let mut module_to_feature_map = HashMap::new();

        for (pkg, mapping) in &config.modules {
            package_to_module.insert(pkg.clone(), mapping.module.clone());
            module_to_feature_map.insert(mapping.module.clone(), mapping.feature.clone());
        }

        // Build enabled modules from features
        let enabled_features: HashSet<&str> = config.features.iter().map(|s| s.as_str()).collect();
        let mut enabled_modules = HashSet::new();
        for (pkg, mapping) in &config.modules {
            if enabled_features.contains(mapping.feature.as_str()) {
                enabled_modules.insert(mapping.module.clone());
                // Also ensure the package mapping exists for auto-derived packages
                package_to_module.entry(pkg.clone()).or_insert_with(|| mapping.module.clone());
            }
        }

        // Auto-derive module names for packages not in config
        let mut all_packages = HashSet::new();
        for c in &classes {
            all_packages.insert(c.package.clone());
        }
        for s in &structs {
            all_packages.insert(s.package.clone());
        }
        for e in &enums {
            all_packages.insert(e.package.clone());
        }

        for pkg in &all_packages {
            if !package_to_module.contains_key(pkg) {
                let module = crate::naming::to_snake_case(pkg);
                package_to_module.insert(pkg.clone(), module);
            }
        }

        // Group types by module
        let mut module_classes: BTreeMap<String, Vec<ClassInfo>> = BTreeMap::new();
        let mut module_structs: BTreeMap<String, Vec<StructInfo>> = BTreeMap::new();
        let mut module_enums: BTreeMap<String, Vec<EnumInfo>> = BTreeMap::new();

        let mut classes_map = HashMap::new();
        for c in classes {
            if let Some(module) = package_to_module.get(&c.package) {
                if enabled_modules.contains(module) {
                    classes_map.insert(c.name.clone(), c.clone());
                    module_classes.entry(module.clone()).or_default().push(c);
                }
            }
        }

        let mut structs_map = HashMap::new();
        for s in structs {
            if let Some(module) = package_to_module.get(&s.package) {
                if enabled_modules.contains(module) {
                    structs_map.insert(s.name.clone(), s.clone());
                    module_structs.entry(module.clone()).or_default().push(s);
                }
            }
        }

        let mut enums_map = HashMap::new();
        for e in enums {
            if let Some(module) = package_to_module.get(&e.package) {
                if enabled_modules.contains(module) {
                    enums_map.insert(e.name.clone(), e.clone());
                    module_enums.entry(module.clone()).or_default().push(e);
                }
            }
        }

        // Sort within each module for determinism
        for classes in module_classes.values_mut() {
            classes.sort_by(|a, b| a.name.cmp(&b.name));
        }
        for structs in module_structs.values_mut() {
            structs.sort_by(|a, b| a.name.cmp(&b.name));
        }
        for enums in module_enums.values_mut() {
            enums.sort_by(|a, b| a.name.cmp(&b.name));
        }

        CodegenContext {
            classes: classes_map,
            structs: structs_map,
            enums: enums_map,
            package_to_module,
            module_to_feature: module_to_feature_map,
            enabled_modules,
            module_classes,
            module_structs,
            module_enums,
            func_table: Vec::new(),
        }
    }

    /// Check if a type (class, struct, or enum) exists in an enabled module.
    #[allow(dead_code)]
    pub fn is_type_available(&self, type_name: &str) -> bool {
        self.classes.contains_key(type_name)
            || self.structs.contains_key(type_name)
            || self.enums.contains_key(type_name)
    }

    /// Get the module name for a package.
    #[allow(dead_code)]
    pub fn module_for_package(&self, package: &str) -> Option<&str> {
        self.package_to_module.get(package).map(|s| s.as_str())
    }

    /// Get the Cargo feature name for a module.
    pub fn feature_for_module(&self, module: &str) -> Option<&str> {
        self.module_to_feature.get(module).map(|s| s.as_str())
    }

    /// Get the actual Rust repr type for an enum, accounting for signed promotion.
    /// This must match the logic in `rust_gen/enums.rs`.
    pub fn enum_actual_repr(&self, enum_name: &str) -> Option<&'static str> {
        let e = self.enums.get(enum_name)?;
        let has_negative = e.pairs.iter().any(|(_, v)| *v < 0);
        Some(if has_negative {
            match e.underlying_type.as_str() {
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
        } else {
            match e.underlying_type.as_str() {
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
        })
    }
}
