# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Uika** is a Rust binding for Unreal Engine 5.7+. Rust compiles to a DLL loaded by a UE C++ plugin. All UE calls cross the FFI boundary through a function pointer table (`UikaApiTable`). The Rust side uses idiomatic Rust patterns.

## Architecture

### Dual-path function calling

- **Direct call path** (primary, 99%): codegen generates C++ wrapper functions that directly call UE C++ APIs. Pointers to these wrappers are stored in a flat `func_table` array indexed by codegen-assigned `FuncId`. Rust calls these via `transmute`.
- **Reflection path** (fallback): `DynamicCall` API uses `find_function` → `alloc_params` → `call_function` (ProcessEvent) for Blueprint-defined or dynamically discovered functions.

### Crate structure (planned)

| Crate | Role |
|-------|------|
| `uika-ffi` | `#[repr(C)]` types, handle types, API table definition. Zero dependencies. |
| `uika-runtime` | Safe Rust API: `UObjectRef<T>`, `Pinned<T>`, `DynamicCall`, error types, traits. All `unsafe` confined here. |
| `uika-macros` | Proc macros: `#[uclass]`, `#[ufunction]`, `#[uproperty]` |
| `uika-codegen` | CLI binary: reads UHT JSON, generates Rust bindings + C++ wrapper functions + FuncId tables |
| `uika-bindings` | Generated code (gitignored). All UE type bindings. |
| `uika` | cdylib entry: `uika_init`/`uika_shutdown`, re-exports |

### UE plugin structure (planned)

- `ue_plugin/Uika/` — Runtime C++ plugin: DLL loading, API table filling, delegate proxy, reified class support
- `ue_plugin/UikaGenerator/` — C# UHT exporter: exports reflection data to JSON

### Lifecycle model

Two-tier references with no global registry:
- `UObjectRef<T>`: 8-byte Copy handle, may become invalid (checks validity via C++ side)
- `Pinned<T>`: RAII GC root, guarantees object stays alive until dropped

### Build flow

```
1. UE build (triggers UikaGenerator → JSON)
2. uika-codegen (JSON → Rust bindings + C++ wrappers + FuncId tables)
3. UE rebuild (compiles generated C++ wrappers into UE module)
4. cargo build --release -p uika (compiles Rust DLL)
5. Copy uika.dll to UE Binaries/
```

### Codegen command

```bash
cargo run -p uika-cli -- generate
```

Codegen reads all configuration (paths, features, module mappings, blocklists) from `uika.config.toml` in the working directory. Override with `--config path/to/config.toml`.

Verify after codegen:
- `cargo build -p uika-bindings --features core,engine` — Rust bindings compile
- Spot-check `ue_plugin/Uika/Source/Uika/Generated/` for correct C++ wrapper patterns

## Rules

### Testing policy

- **No unit tests for codegen** (`uika-codegen`). The only acceptance criterion for codegen output is that both Rust and UE compile successfully and run correctly.
- **All UE-related testing requires a real UE environment.** Do not create mocks or stubs for UE APIs.
- Pure Rust logic tests (type conversion, error handling in `uika-runtime`) are fine as standard `cargo test`.

### Language

- All code, comments, and commit messages in English.
