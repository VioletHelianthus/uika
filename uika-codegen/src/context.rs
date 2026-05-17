// Build context: type lookups, module mapping, FuncId assignment.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::config::CodegenConfig;
use crate::schema::{ClassInfo, EnumInfo, FunctionInfo, PropertyInfo, StructInfo};

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

    /// Module name → set of other modules whose types it references.
    /// Drives feature dependency emission in the generated `uika-bindings/Cargo.toml`.
    pub module_deps: BTreeMap<String, std::collections::BTreeSet<String>>,
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

        let mut ctx = CodegenContext {
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
            module_deps: BTreeMap::new(),
        };
        ctx.module_deps = ctx.compute_module_deps();
        ctx
    }

    /// Walk all enabled types and record cross-module type references.
    ///
    /// For each (module M, type T in M), if T references a type owned by
    /// another module M', record `M depends on M'`. The resulting graph
    /// drives feature dependency emission in `uika-bindings/Cargo.toml`,
    /// ensuring `cargo check` succeeds at any enabled feature combination.
    fn compute_module_deps(&self) -> BTreeMap<String, std::collections::BTreeSet<String>> {
        let mut deps: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
        for m in &self.enabled_modules {
            deps.entry(m.clone()).or_default();
        }

        for (module, classes) in &self.module_classes {
            for class in classes {
                if let Some(parent) = &class.super_class {
                    if let Some(pc) = self.classes.get(parent.as_str()) {
                        self.record_dep(module, &pc.package, &mut deps);
                    }
                }
                for iface in &class.interfaces {
                    if let Some(ic) = self.classes.get(iface.as_str()) {
                        self.record_dep(module, &ic.package, &mut deps);
                    }
                }
                for prop in &class.props {
                    self.walk_prop_deps(prop, module, &mut deps);
                }
                for func in &class.funcs {
                    for p in &func.params {
                        self.walk_param_deps(p, module, &mut deps);
                    }
                }
            }
        }

        for (module, structs) in &self.module_structs {
            for s in structs {
                if let Some(parent) = &s.super_struct {
                    if let Some(ps) = self.structs.get(parent.as_str()) {
                        self.record_dep(module, &ps.package, &mut deps);
                    }
                }
                for prop in &s.props {
                    self.walk_prop_deps(prop, module, &mut deps);
                }
            }
        }

        deps
    }

    fn record_dep(
        &self,
        current_module: &str,
        target_package: &str,
        deps: &mut BTreeMap<String, std::collections::BTreeSet<String>>,
    ) {
        if let Some(target_module) = self.package_to_module.get(target_package) {
            if target_module != current_module && self.enabled_modules.contains(target_module) {
                deps.entry(current_module.to_string())
                    .or_default()
                    .insert(target_module.clone());
            }
        }
    }

    fn walk_prop_deps(
        &self,
        prop: &PropertyInfo,
        current_module: &str,
        deps: &mut BTreeMap<String, std::collections::BTreeSet<String>>,
    ) {
        if let Some(en) = &prop.enum_name {
            if let Some(e) = self.enums.get(en.as_str()) {
                self.record_dep(current_module, &e.package, deps);
            }
        }
        if let Some(cn) = &prop.class_name {
            if let Some(c) = self.classes.get(cn.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(mc) = &prop.meta_class_name {
            if let Some(c) = self.classes.get(mc.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(sn) = &prop.struct_name {
            if let Some(s) = self.structs.get(sn.as_str()) {
                self.record_dep(current_module, &s.package, deps);
            }
        }
        if let Some(in_) = &prop.interface_name {
            if let Some(c) = self.classes.get(in_.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(fi) = &prop.func_info {
            self.walk_delegate_func_info_deps(fi, current_module, deps);
        }
        if let Some(inner) = &prop.inner_prop {
            self.walk_prop_deps(inner, current_module, deps);
        }
        if let Some(key) = &prop.key_prop {
            self.walk_prop_deps(key, current_module, deps);
        }
        if let Some(value) = &prop.value_prop {
            self.walk_prop_deps(value, current_module, deps);
        }
        if let Some(element) = &prop.element_prop {
            self.walk_prop_deps(element, current_module, deps);
        }
    }

    fn walk_param_deps(
        &self,
        param: &crate::schema::ParamInfo,
        current_module: &str,
        deps: &mut BTreeMap<String, std::collections::BTreeSet<String>>,
    ) {
        if let Some(en) = &param.enum_name {
            if let Some(e) = self.enums.get(en.as_str()) {
                self.record_dep(current_module, &e.package, deps);
            }
        }
        if let Some(cn) = &param.class_name {
            if let Some(c) = self.classes.get(cn.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(mc) = &param.meta_class_name {
            if let Some(c) = self.classes.get(mc.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(sn) = &param.struct_name {
            if let Some(s) = self.structs.get(sn.as_str()) {
                self.record_dep(current_module, &s.package, deps);
            }
        }
        if let Some(in_) = &param.interface_name {
            if let Some(c) = self.classes.get(in_.as_str()) {
                self.record_dep(current_module, &c.package, deps);
            }
        }
        if let Some(inner) = &param.inner_prop {
            self.walk_prop_deps(inner, current_module, deps);
        }
        if let Some(key) = &param.key_prop {
            self.walk_prop_deps(key, current_module, deps);
        }
        if let Some(value) = &param.value_prop {
            self.walk_prop_deps(value, current_module, deps);
        }
        if let Some(element) = &param.element_prop {
            self.walk_prop_deps(element, current_module, deps);
        }
    }

    fn walk_delegate_func_info_deps(
        &self,
        fi: &serde_json::Value,
        current_module: &str,
        deps: &mut BTreeMap<String, std::collections::BTreeSet<String>>,
    ) {
        let Some(params) = fi.get("params").and_then(|p| p.as_array()) else { return };
        for p in params {
            if let Some(en) = p.get("enum_name").and_then(|v| v.as_str()) {
                if let Some(e) = self.enums.get(en) {
                    self.record_dep(current_module, &e.package, deps);
                }
            }
            if let Some(cn) = p.get("class_name").and_then(|v| v.as_str()) {
                if let Some(c) = self.classes.get(cn) {
                    self.record_dep(current_module, &c.package, deps);
                }
            }
            if let Some(mc) = p.get("meta_class_name").and_then(|v| v.as_str()) {
                if let Some(c) = self.classes.get(mc) {
                    self.record_dep(current_module, &c.package, deps);
                }
            }
            if let Some(sn) = p.get("struct_name").and_then(|v| v.as_str()) {
                if let Some(s) = self.structs.get(sn) {
                    self.record_dep(current_module, &s.package, deps);
                }
            }
            if let Some(in_) = p.get("interface_name").and_then(|v| v.as_str()) {
                if let Some(c) = self.classes.get(in_) {
                    self.record_dep(current_module, &c.package, deps);
                }
            }
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
