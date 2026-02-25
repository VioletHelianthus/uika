// Sync command: copies hand-written UE plugin files from workspace ue_plugin/
// into uika-cli/ue_plugin_embed/ for crates.io packaging.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Directories to skip when walking plugin sources.
const EXCLUDED_DIRS: &[&str] = &["Generated", "Binaries", "Intermediate", "obj"];

pub fn run_sync() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_dir.join("..").join("ue_plugin");
    let dest_root = manifest_dir.join("ue_plugin_embed");

    if !source_root.join("Uika").exists() {
        eprintln!(
            "Error: source directory not found: {}/Uika/",
            source_root.display()
        );
        eprintln!("This command must be run from within the uika workspace.");
        std::process::exit(1);
    }

    // Collect source files
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(&source_root, &source_root, &mut files);

    // Track which destination files we write (for stale cleanup)
    let mut written_paths: HashSet<PathBuf> = HashSet::new();

    eprintln!("uika sync-plugin: syncing {} files...", files.len());

    for (rel_path, src_path) in &files {
        let dest_path = dest_root.join(rel_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("Failed to create {}: {e}", parent.display()));
        }

        let src_contents = fs::read(src_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", src_path.display()));

        // Only write if contents differ (avoid unnecessary timestamp changes)
        let needs_write = match fs::read(&dest_path) {
            Ok(existing) => existing != src_contents,
            Err(_) => true,
        };

        if needs_write {
            fs::write(&dest_path, &src_contents)
                .unwrap_or_else(|e| panic!("Failed to write {}: {e}", dest_path.display()));
            eprintln!("  updated: {rel_path}");
        }

        written_paths.insert(dest_path);
    }

    // Clean stale files in dest that don't exist in source
    if dest_root.exists() {
        let mut stale: Vec<PathBuf> = Vec::new();
        collect_all_files(&dest_root, &mut stale);
        for path in stale {
            if !written_paths.contains(&path) {
                eprintln!("  removed stale: {}", path.strip_prefix(&dest_root).unwrap().display());
                let _ = fs::remove_file(&path);
            }
        }

        // Clean empty directories
        clean_empty_dirs(&dest_root);
    }

    eprintln!("uika sync-plugin: done! ({} files)", files.len());
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

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
            if name.ends_with(".csproj.props") {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap();
            let rel_str = rel.to_str().expect("non-UTF8 path").replace('\\', "/");
            out.push((rel_str, path.clone()));
        }
    }
}

fn collect_all_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_all_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn clean_empty_dirs(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            clean_empty_dirs(&path);
            // Try to remove — will fail if non-empty, which is fine
            let _ = fs::remove_dir(&path);
        }
    }
}
