// Build command: 5-step build pipeline replacing tools/build.py.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use uika_codegen::config::UikaConfig;

/// Run the build pipeline.
///
/// `config_path` is the path to uika.config.toml.
/// `step` runs only that step (1-5). `from` starts from that step (1-5).
/// `step` and `from` are mutually exclusive.
pub fn run_build(config_path: &Path, step: Option<u8>, from: u8) {
    // Validate step/from
    if step.is_some() && from != 1 {
        eprintln!("Error: --step and --from are mutually exclusive.");
        std::process::exit(1);
    }
    if let Some(s) = step {
        if !(1..=5).contains(&s) {
            eprintln!("Error: --step must be 1-5, got {s}");
            std::process::exit(1);
        }
    }
    if !(1..=5).contains(&from) {
        eprintln!("Error: --from must be 1-5, got {from}");
        std::process::exit(1);
    }

    // Load config
    let config_str = fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", config_path.display()));
    let config: UikaConfig = toml::from_str(&config_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", config_path.display()));

    // Resolve paths relative to config file directory
    let config_parent = config_path.parent().unwrap_or(Path::new("."));
    let config_dir = if config_parent.as_os_str().is_empty() {
        Path::new(".").canonicalize()
    } else {
        config_parent.canonicalize()
    }
    .unwrap_or_else(|e| panic!("Failed to canonicalize config dir: {e}"));

    let ctx = BuildContext::new(&config, &config_dir, config_path);

    // Determine which steps to run
    let steps: Vec<u8> = if let Some(s) = step {
        vec![s]
    } else {
        (from..=5).collect()
    };

    let step_descs: &[&str] = &[
        "",
        "UE build (UHT export to JSON)",
        "uika-codegen (JSON -> Rust + C++)",
        "UE rebuild (compile C++ wrappers)",
        "cargo build --release (cdylib)",
        "Copy DLL to plugin Binaries as uika.dll",
    ];

    let total = steps.len();
    let overall_start = Instant::now();

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("  Uika build pipeline");
    if step.is_some() {
        eprintln!("  Running step {} only", steps[0]);
    } else if from > 1 {
        eprintln!("  Running steps {from}-5");
    } else {
        eprintln!("  Running all {total} steps");
    }
    eprintln!("{}\n", "=".repeat(60));

    for (i, &step_num) in steps.iter().enumerate() {
        let step_start = Instant::now();
        eprintln!(
            "[{}/{}] Step {}: {}",
            i + 1,
            total,
            step_num,
            step_descs[step_num as usize]
        );
        eprintln!("{}", "-".repeat(60));

        match step_num {
            1 => ctx.step1_ue_build(),
            2 => ctx.step2_codegen(),
            3 => ctx.step3_ue_rebuild(),
            4 => ctx.step4_cargo_build(),
            5 => ctx.step5_copy_dll(),
            _ => unreachable!(),
        }

        let elapsed = step_start.elapsed().as_secs_f64();
        eprintln!("  Step {step_num} completed in {elapsed:.1}s\n");
    }

    let overall = overall_start.elapsed().as_secs_f64();
    eprintln!("{}", "=".repeat(60));
    eprintln!("  All steps completed in {overall:.1}s");
    eprintln!("{}", "=".repeat(60));
}

/// Resolved build context with all paths pre-computed.
struct BuildContext {
    engine_path: PathBuf,
    project_path: PathBuf,
    uht_input: PathBuf,
    config_path: PathBuf,
    crate_name: String,
    config_dir: PathBuf,
}

impl BuildContext {
    fn new(config: &UikaConfig, config_dir: &Path, config_path: &Path) -> Self {
        // Engine path (required for build)
        let engine_path = config
            .ue
            .as_ref()
            .map(|ue| PathBuf::from(&ue.engine_path))
            .unwrap_or_else(|| {
                eprintln!("Error: [ue].engine_path is required for build.");
                std::process::exit(1);
            });

        // Project path (relative to config dir)
        let project_rel = config
            .project
            .as_ref()
            .map(|p| p.path.as_str())
            .unwrap_or(".");
        let project_path = config_dir.join(project_rel);

        // UHT input path (relative to config dir)
        let uht_input = config_dir.join(&config.codegen.paths.uht_input);

        // Crate name: from config or auto-detect
        let crate_name = config
            .build
            .as_ref()
            .and_then(|b| b.crate_name.clone())
            .unwrap_or_else(|| detect_cdylib_crate(config_dir));

        BuildContext {
            engine_path,
            project_path,
            uht_input,
            config_path: config_path.to_path_buf(),
            crate_name,
            config_dir: config_dir.to_path_buf(),
        }
    }

    /// Find the .uproject file and derive the Editor target name.
    fn editor_target(&self) -> String {
        let uproject = find_uproject(&self.project_path).unwrap_or_else(|| {
            eprintln!(
                "Error: no .uproject found in {}",
                self.project_path.display()
            );
            std::process::exit(1);
        });
        let stem = uproject
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();
        format!("{stem}Editor")
    }

    /// Full path to the .uproject file.
    fn uproject_path(&self) -> PathBuf {
        find_uproject(&self.project_path).unwrap_or_else(|| {
            eprintln!(
                "Error: no .uproject found in {}",
                self.project_path.display()
            );
            std::process::exit(1);
        })
    }

    /// Step 1: UE build — triggers UHT export, then copies JSON.
    fn step1_ue_build(&self) {
        let build_bat = self
            .engine_path
            .join("Engine/Build/BatchFiles/Build.bat");
        if !build_bat.exists() {
            eprintln!("Error: Build.bat not found at {}", build_bat.display());
            std::process::exit(1);
        }

        let target = self.editor_target();
        let uproject = self.uproject_path();
        let uproject_abs = uproject.canonicalize().unwrap_or(uproject.clone());

        run_cmd(&[
            build_bat.to_str().unwrap(),
            &target,
            "Win64",
            "Development",
            &format!("-Project={}", uproject_abs.display()),
        ]);

        // Copy UHT JSON files
        let uht_intermediate = self
            .project_path
            .join("Plugins/UikaGenerator/Intermediate/Build/Win64/UnrealEditor/Inc/UikaGenerator/UHT");

        fs::create_dir_all(&self.uht_input).unwrap_or_else(|e| {
            panic!("Failed to create {}: {e}", self.uht_input.display())
        });

        let json_files = [
            "uika_classes.json",
            "uika_structs.json",
            "uika_enums.json",
        ];
        let mut copied = 0;
        for name in &json_files {
            let src = uht_intermediate.join(name);
            if src.exists() {
                let size_kb = fs::metadata(&src).map(|m| m.len() / 1024).unwrap_or(0);
                fs::copy(&src, self.uht_input.join(name)).unwrap_or_else(|e| {
                    panic!("Failed to copy {}: {e}", src.display())
                });
                eprintln!("  Copied {name} ({size_kb} KB)");
                copied += 1;
            } else {
                eprintln!("  Warning: {name} not found in {}", uht_intermediate.display());
            }
        }
        eprintln!(
            "  {copied}/{} JSON files copied to {}",
            json_files.len(),
            self.uht_input.display()
        );
    }

    /// Step 2: Run codegen (in-process).
    fn step2_codegen(&self) {
        uika_codegen::run_generate(&self.config_path);
    }

    /// Step 3: UE rebuild — compiles generated C++ wrappers.
    fn step3_ue_rebuild(&self) {
        let build_bat = self
            .engine_path
            .join("Engine/Build/BatchFiles/Build.bat");
        if !build_bat.exists() {
            eprintln!("Error: Build.bat not found at {}", build_bat.display());
            std::process::exit(1);
        }

        let target = self.editor_target();
        let uproject = self.uproject_path();
        let uproject_abs = uproject.canonicalize().unwrap_or(uproject.clone());

        run_cmd(&[
            build_bat.to_str().unwrap(),
            &target,
            "Win64",
            "Development",
            &format!("-Project={}", uproject_abs.display()),
        ]);
    }

    /// Step 4: cargo build --release.
    fn step4_cargo_build(&self) {
        run_cmd(&[
            "cargo",
            "build",
            "--release",
            "-p",
            &self.crate_name,
        ]);
    }

    /// Step 5: Copy built DLL to UE plugin Binaries.
    fn step5_copy_dll(&self) {
        // Cargo converts hyphens to underscores in output filenames
        let dll_filename = format!("{}.dll", self.crate_name.replace('-', "_"));
        let src = self.config_dir.join("target/release").join(&dll_filename);

        let dest_dir = self
            .project_path
            .join("Plugins/Uika/Binaries/Win64");
        let dest = dest_dir.join("uika.dll");

        if !src.exists() {
            eprintln!("Error: DLL not found at {}", src.display());
            eprintln!("  Did step 4 (cargo build) succeed?");
            std::process::exit(1);
        }

        fs::create_dir_all(&dest_dir)
            .unwrap_or_else(|e| panic!("Failed to create {}: {e}", dest_dir.display()));
        fs::copy(&src, &dest)
            .unwrap_or_else(|e| panic!("Failed to copy DLL: {e}"));

        eprintln!("  Copied {}", src.display());
        eprintln!("      -> {} (renamed to uika.dll)", dest.display());
    }
}

/// Run an external command, printing it and exiting on failure.
fn run_cmd(args: &[&str]) {
    let display: String = args.iter().map(|a| *a).collect::<Vec<_>>().join(" ");
    let truncated = if display.len() > 200 {
        format!("{}...", &display[..197])
    } else {
        display
    };
    eprintln!("  $ {truncated}");

    let status = Command::new(args[0])
        .args(&args[1..])
        .status()
        .unwrap_or_else(|e| panic!("Failed to run {}: {e}", args[0]));

    if !status.success() {
        let code = status.code().unwrap_or(1);
        eprintln!("\n  Command failed with exit code {code}");
        std::process::exit(code);
    }
}

/// Find a .uproject file in the given directory.
fn find_uproject(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "uproject") {
            return Some(path);
        }
    }
    None
}

/// Auto-detect the cdylib crate name from workspace Cargo.toml files.
fn detect_cdylib_crate(config_dir: &Path) -> String {
    // Walk the workspace looking for a crate with crate-type = ["cdylib"]
    let workspace_toml = config_dir.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&workspace_toml) {
        if let Ok(doc) = content.parse::<toml::Table>() {
            // Check workspace members
            if let Some(workspace) = doc.get("workspace").and_then(|w| w.as_table()) {
                if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
                    for member in members {
                        if let Some(member_str) = member.as_str() {
                            let member_toml = config_dir.join(member_str).join("Cargo.toml");
                            if let Some(name) = check_cdylib_crate(&member_toml) {
                                return name;
                            }
                        }
                    }
                }
            }
            // Check if this is a single-crate project
            if let Some(name) = check_cdylib_crate(&workspace_toml) {
                return name;
            }
        }
    }

    eprintln!("Warning: could not auto-detect cdylib crate name, using 'uika'.");
    "uika".to_string()
}

/// Check if a Cargo.toml defines a cdylib crate, return its name.
fn check_cdylib_crate(cargo_toml: &Path) -> Option<String> {
    let content = fs::read_to_string(cargo_toml).ok()?;
    let doc: toml::Table = content.parse().ok()?;

    // Check [lib].crate-type for "cdylib"
    let lib = doc.get("lib")?.as_table()?;
    let crate_type = lib.get("crate-type").or_else(|| lib.get("crate_type"))?;
    let types = crate_type.as_array()?;
    let is_cdylib = types.iter().any(|t| t.as_str() == Some("cdylib"));

    if !is_cdylib {
        return None;
    }

    // Get crate name: [lib].name or [package].name
    let name = lib
        .get("name")
        .and_then(|n| n.as_str())
        .or_else(|| {
            doc.get("package")
                .and_then(|p| p.as_table())
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
        })?;

    Some(name.to_string())
}
