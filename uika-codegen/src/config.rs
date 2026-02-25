// Configuration types for uika-codegen, deserialized from uika.config.toml.

use serde::Deserialize;
use std::collections::HashMap;

/// Top-level config file.
#[derive(Deserialize)]
pub struct UikaConfig {
    pub codegen: CodegenConfig,
    #[serde(default)]
    pub ue: Option<UeConfig>,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub project: Option<ProjectConfig>,
}

#[derive(Deserialize)]
pub struct UeConfig {
    pub engine_path: String,
}

#[derive(Deserialize)]
pub struct BuildConfig {
    /// Cargo crate name of the cdylib to build. If not set, auto-detected from Cargo.toml.
    pub crate_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ProjectConfig {
    /// Path to the UE project directory (relative to config file location).
    /// Defaults to "." (current directory).
    #[serde(default = "default_project_path")]
    pub path: String,
}

fn default_project_path() -> String {
    ".".to_string()
}

#[derive(Deserialize)]
pub struct CodegenConfig {
    pub features: Vec<String>,
    pub paths: CodegenPaths,
    pub modules: HashMap<String, ModuleMapping>,
    pub blocklist: Blocklist,
}

#[derive(Deserialize)]
pub struct CodegenPaths {
    pub uht_input: String,
    pub rust_out: String,
    pub cpp_out: String,
}

#[derive(Deserialize)]
pub struct ModuleMapping {
    pub module: String,
    pub feature: String,
}

#[derive(Deserialize)]
pub struct Blocklist {
    pub classes: Vec<String>,
    pub structs: Vec<String>,
    /// Function blocklist in "Class.Function" format.
    pub functions: Vec<String>,
}

impl Blocklist {
    /// Parse function blocklist entries into (class, function) tuples.
    pub fn function_tuples(&self) -> Vec<(String, String)> {
        self.functions
            .iter()
            .filter_map(|entry| {
                let (class, func) = entry.split_once('.')?;
                Some((class.to_string(), func.to_string()))
            })
            .collect()
    }
}
