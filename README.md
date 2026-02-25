# Uika

**Rust bindings for Unreal Engine 5.7+**

Uika lets you write Unreal Engine gameplay in Rust. Your Rust code compiles to a DLL that is loaded by a small UE C++ plugin. All UE API calls cross the FFI boundary through a function pointer table — no C++ compilation required during Rust iteration.

> **⚠️ Early Stage Project** — Uika is under active development and **not ready for production use**. APIs will change without notice, documentation is incomplete, and many UE features are not yet covered. Contributions and feedback are welcome, but please do not use this for shipping projects.

## Highlights

- **Define UE classes in Rust** — `#[uclass]`, `#[ufunction]`, `#[uproperty]` proc macros with full Blueprint integration
- **Type-safe UE API access** — generated bindings for 12+ UE modules with idiomatic Rust signatures
- **Two-tier object lifecycle** — `UObjectRef<T>` (lightweight handle) and `Pinned<T>` (RAII GC root) with no global registry overhead
- **Hot reload** — `Uika.Reload` console command swaps your DLL without restarting the editor
- **Dual-path function calling** — direct C++ wrapper calls (99% path) with reflection fallback for Blueprint-defined functions
- **Cargo features** — opt into only the UE modules you need (`core`, `engine`, `physics-core`, `umg`, `niagara`, etc.)

## Example

A complete game in Rust — top-down gem collector with score, countdown timer, and HUD:

```rust
use uika::{uclass, uclass_impl};
use uika::runtime::{ulog, Checked, OwnedStruct, UObjectRef, UikaResult, LOG_DISPLAY};
use uika::bindings::core_ue::{FTransform, FRotator, FRotatorExt};
use uika::bindings::engine::*;
use uika::bindings::manual::world_ext::WorldSpawnExt;
use uika::runtime::Transform;
use glam::{DQuat, DVec3};

#[uclass(parent = DefaultPawn)]
pub struct GemCollectorPawn {
    #[uproperty(BlueprintReadWrite)]
    score: i32,

    #[uproperty(BlueprintReadWrite, default = 400.0)]
    move_speed: f32,

    #[uproperty(BlueprintReadWrite, default = 60.0)]
    time_remaining: f32,

    // Rust-private fields (not exposed to UE)
    game_over: bool,
}

#[uclass_impl]
impl GemCollectorPawn {
    #[ufunction(Override)]
    fn receive_begin_play(&mut self) {
        ulog!(LOG_DISPLAY, "Game started!");
    }

    #[ufunction(Override)]
    fn receive_tick(&mut self, delta_seconds: f32) {
        let time = (self.time_remaining() - delta_seconds).max(0.0);
        self.set_time_remaining(time);
        if time <= 0.0 {
            self.set_game_over(true);
            ulog!(LOG_DISPLAY, "Game Over! Final score: {}", self.score());
        }
    }

    #[ufunction(BlueprintCallable)]
    fn get_score(&self) -> i32 {
        self.score()
    }
}
```

See [`example_game/src/game_demo.rs`](example_game/src/game_demo.rs) for the full working demo.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    Your Rust Code                     │
│            (cdylib using uika crate)                  │
├──────────────────────────────────────────────────────┤
│  uika              │  uika-bindings (generated)       │
│  - entry!() macro  │  - Class/struct/enum bindings     │
│  - prelude         │  - Property getters/setters       │
│  - re-exports      │  - Function wrappers              │
├────────────────────┼──────────────────────────────────┤
│  uika-runtime      │  uika-macros                      │
│  - UObjectRef<T>   │  - #[uclass]                      │
│  - Pinned<T>       │  - #[ufunction]                   │
│  - DynamicCall     │  - #[uproperty]                   │
│  - Containers      │  - #[component]                   │
├────────────────────┴──────────────────────────────────┤
│  uika-ffi          │  uika-ue-flags                    │
│  - #[repr(C)]      │  - CPF_*, FUNC_*, CLASS_*         │
│  - UikaApiTable    │    flag constants                  │
╞══════════════════════════════════════════════════════╡
│              FFI boundary (func_table)                │
├──────────────────────────────────────────────────────┤
│  UE C++ Plugin (ue_plugin/Uika/)                      │
│  - DLL loading, API table, delegate proxy             │
│  - Generated C++ wrappers (direct call path)          │
│  - Reified class support (UUikaReifiedClass)          │
├──────────────────────────────────────────────────────┤
│  UHT Exporter (ue_plugin/UikaGenerator/)              │
│  - C# plugin for Unreal Header Tool                   │
│  - Exports reflection data to JSON                    │
├──────────────────────────────────────────────────────┤
│              Unreal Engine 5.7+                        │
└──────────────────────────────────────────────────────┘
```

### Crate structure

| Crate | Role |
|-------|------|
| **`uika`** | User-facing library. Re-exports everything, provides `entry!()` macro. |
| **`uika-ffi`** | `#[repr(C)]` types, handle types, API table definition. Zero dependencies. |
| **`uika-runtime`** | Safe Rust API: `UObjectRef<T>`, `Pinned<T>`, `DynamicCall`, containers, error types. All `unsafe` confined here. |
| **`uika-macros`** | Proc macros: `#[uclass]`, `#[ufunction]`, `#[uproperty]`, `#[component]`. |
| **`uika-codegen`** | Library: reads UHT JSON, generates Rust bindings + C++ wrapper functions. |
| **`uika-cli`** | CLI binary: `generate`, `setup`, `build`, `sync-plugin` commands. |
| **`uika-bindings`** | Generated code (gitignored). All UE type bindings. |
| **`uika-ue-flags`** | UE reflection flag constants shared across crates. |

## Getting Started

### Prerequisites

- **Unreal Engine 5.7+** (source or installed build)
- **Rust** (stable, latest recommended)
- **Visual Studio 2022** with C++ workload (for UE compilation)

### Setup

1. **Create your UE project** (or use an existing one).

2. **Clone this repo** alongside your project:
   ```bash
   git clone https://github.com/user/uika.git
   cd uika
   ```

3. **Configure** — copy and edit the config file:
   ```bash
   cp uika.config.toml.example uika.config.toml
   ```
   Edit `uika.config.toml` to set your UE engine path and project path:
   ```toml
   [ue]
   engine_path = "C:/Program Files/Epic Games/UE_5.7"

   [project]
   path = "your_project"

   [build]
   crate_name = "your-game"
   ```

4. **Set up the UE plugin** in your project:
   ```bash
   cargo run -p uika-cli -- setup
   ```

5. **Build everything** (UE build → codegen → UE rebuild → Rust compile → deploy DLL):
   ```bash
   cargo run -p uika-cli -- build
   ```

### Create your game crate

```bash
cargo new --lib your-game
```

**`Cargo.toml`:**
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
uika = { path = "../uika", features = ["engine"] }
glam = "0.29"
```

**`src/lib.rs`:**
```rust
uika::entry!();

mod my_game;
```

## Build Pipeline

The CLI orchestrates a 5-step build:

| Step | Command | What it does |
|------|---------|-------------|
| 1 | UE Build | Compiles UE project, triggers UikaGenerator → JSON reflection data |
| 2 | Codegen | Reads JSON → generates Rust bindings + C++ wrappers |
| 3 | UE Rebuild | Compiles the generated C++ wrappers into the UE module |
| 4 | Cargo Build | `cargo build --release` on your cdylib crate |
| 5 | Deploy | Copies the DLL to `Plugins/Uika/Binaries/Win64/` |

Common shortcuts:
```bash
# Full build
cargo run -p uika-cli -- build

# Rust-only rebuild (skip UE steps)
cargo run -p uika-cli -- build --from 4

# Codegen + everything after
cargo run -p uika-cli -- build --from 2

# Just regenerate bindings
cargo run -p uika-cli -- generate
```

## Cargo Features

Only pull in the UE modules you need:

| Feature | UE Module | Depends on |
|---------|-----------|------------|
| `core` | CoreUObject | — |
| `engine` | Engine | `core` |
| `physics-core` | PhysicsCore | `engine` |
| `input` | InputCore | `core` |
| `slate` | SlateCore + Slate | `core` |
| `umg` | UMG | `slate` |
| `niagara` | Niagara | `engine` |
| `gameplay-abilities` | GameplayAbilities | `engine` |
| `level-sequence` | LevelSequence | `engine` |
| `cinematic` | CinematicCamera | `engine` |
| `movie` | MovieScene + MovieSceneTracks | `engine` |

Default features: `core`, `engine`.

## Key Concepts

### Object References

```rust
// Lightweight handle (8 bytes, Copy). May become invalid if UE GCs the object.
let actor: UObjectRef<Actor> = world.spawn_actor(&transform)?;

// RAII GC root. Prevents garbage collection until dropped.
let pinned: Pinned<Actor> = actor.pin()?;

// Checked access — verifies the object is still alive before use.
let checked = actor.checked()?;
checked.k2_get_actor_location();
```

### Defining UE Classes

```rust
#[uclass(parent = Actor)]
pub struct MyActor {
    #[component(root)]
    root: SceneComponent,

    #[component(attach = "root")]
    mesh: StaticMeshComponent,

    #[uproperty(BlueprintReadWrite, default = 100)]
    health: i32,

    // Rust-only field (not exposed to UE)
    internal_state: Vec<String>,
}

#[uclass_impl]
impl MyActor {
    #[ufunction(Override)]
    fn receive_begin_play(&mut self) { /* ... */ }

    #[ufunction(BlueprintCallable)]
    fn take_damage(&mut self, amount: i32) {
        self.set_health(self.health() - amount);
    }
}
```

### Dynamic Calls

For Blueprint-defined functions or APIs not covered by generated bindings:

```rust
let mut call = DynamicCall::new(&actor, "SetMobility")?;
call.set("NewMobility", 2u8)?;
call.call()?;
```

### Hot Reload

During development, rebuild your Rust DLL and reload without restarting the editor:

```bash
cargo run -p uika-cli -- build --from 4
```

Then in the UE console:
```
Uika.Reload
```

Function implementations update immediately. Adding/removing `uproperty` or `ufunction` requires an editor restart.

## Platform Support

| Platform | Status |
|----------|--------|
| Windows (x64) | Supported |
| Linux | Not yet tested |
| macOS | Not yet tested |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
