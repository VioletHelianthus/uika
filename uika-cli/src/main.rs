// uika-cli: CLI entry point for Uika tools (codegen, setup, build, sync-plugin).

mod setup;
mod sync_plugin;
mod build_cmd;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use uika_codegen::config::UikaConfig;

#[derive(Parser)]
#[command(name = "uika", about = "Uika CLI — UE binding tools for Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate Rust bindings and C++ wrapper functions from UHT JSON.
    Generate {
        /// Path to uika.config.toml.
        #[arg(long, default_value = "uika.config.toml")]
        config: PathBuf,
    },
    /// Extract UE plugin files into a UE project's Plugins/ directory.
    Setup {
        /// Path to the UE project directory.
        project: PathBuf,
        /// Path to the UE engine root (e.g. "F:/UE_5.7").
        #[arg(long)]
        engine_path: Option<PathBuf>,
        /// Path to uika.config.toml (reads [ue].engine_path as fallback).
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Sync hand-written plugin files into ue_plugin_embed/ for crates.io packaging.
    SyncPlugin,
    /// Run the 5-step build pipeline.
    Build {
        /// Path to uika.config.toml.
        #[arg(long, default_value = "uika.config.toml")]
        config: PathBuf,
        /// Run only step N (1-5).
        #[arg(long)]
        step: Option<u8>,
        /// Start from step N (1-5, default: 1).
        #[arg(long, default_value_t = 1)]
        from: u8,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { project, engine_path, config } => {
            let engine = resolve_engine_path(engine_path, config.as_deref());
            setup::run_setup(&project, &engine);
        }
        Commands::SyncPlugin => {
            sync_plugin::run_sync();
        }
        Commands::Generate { config: config_path } => {
            uika_codegen::run_generate(&config_path);
        }
        Commands::Build { config, step, from } => {
            build_cmd::run_build(&config, step, from);
        }
    }
}

/// Resolve engine path from CLI flag or config file fallback.
fn resolve_engine_path(flag: Option<PathBuf>, config_path: Option<&Path>) -> PathBuf {
    // 1. Explicit --engine-path flag
    if let Some(path) = flag {
        return path;
    }

    // 2. Try reading from config file
    let config_path = config_path.unwrap_or_else(|| Path::new("uika.config.toml"));
    if let Ok(config_str) = std::fs::read_to_string(config_path) {
        if let Ok(config) = toml::from_str::<UikaConfig>(&config_str) {
            if let Some(ue) = config.ue {
                return PathBuf::from(ue.engine_path);
            }
        }
    }

    eprintln!("Error: engine path not specified.");
    eprintln!("Provide --engine-path or set [ue].engine_path in uika.config.toml.");
    std::process::exit(1);
}
