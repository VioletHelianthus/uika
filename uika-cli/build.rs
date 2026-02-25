// Build script: embeds UE plugin source files into the binary via include_bytes!.
//
// Dual-path resolution:
// - Workspace build: reads from ../../ue_plugin/ (always up-to-date)
// - crates.io build: falls back to ./ue_plugin_embed/ (committed snapshot)

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Directories to skip when walking plugin sources.
const EXCLUDED_DIRS: &[&str] = &["Generated", "Binaries", "Intermediate", "obj"];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Dual-path: prefer workspace ue_plugin/, fall back to ue_plugin_embed/
    let workspace_path = manifest_dir.join("..").join("ue_plugin");
    let embed_path = manifest_dir.join("ue_plugin_embed");

    let source_root = if workspace_path.join("Uika").exists() {
        workspace_path
    } else if embed_path.join("Uika").exists() {
        embed_path
    } else {
        panic!(
            "Cannot find UE plugin sources. \
             Expected either {workspace} or {embed} to contain Uika/",
            workspace = workspace_path.display(),
            embed = embed_path.display(),
        );
    };

    let source_root = fs::canonicalize(&source_root)
        .unwrap_or_else(|e| panic!("Failed to canonicalize {}: {e}", source_root.display()));

    // Tell Cargo to rerun if plugin sources change
    println!("cargo:rerun-if-changed={}", source_root.display());

    // Collect all eligible files
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(&source_root, &source_root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate plugin_files.rs
    let out_file = out_dir.join("plugin_files.rs");
    let mut f = fs::File::create(&out_file)
        .unwrap_or_else(|e| panic!("Failed to create {}: {e}", out_file.display()));

    writeln!(f, "pub const PLUGIN_FILES: &[(&str, &[u8])] = &[").unwrap();
    for (rel_path, abs_path) in &files {
        // Use forward slashes for the include_bytes! path (works on all platforms)
        let abs_str = abs_path.to_str().expect("non-UTF8 path").replace('\\', "/");
        writeln!(f, "    ({rel_path:?}, include_bytes!({abs_str:?})),").unwrap();
    }
    writeln!(f, "];").unwrap();

    eprintln!(
        "uika-codegen build.rs: embedded {} plugin files from {}",
        files.len(),
        source_root.display()
    );
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            if EXCLUDED_DIRS.contains(&name.as_ref()) {
                continue;
            }
            collect_files(root, &path, out);
        } else {
            // Skip .csproj.props (machine-specific)
            if name.ends_with(".csproj.props") {
                continue;
            }

            let rel = path.strip_prefix(root).unwrap();
            // Use forward slashes in relative paths
            let rel_str = rel.to_str().expect("non-UTF8 path").replace('\\', "/");
            out.push((rel_str, path.clone()));
        }
    }
}
