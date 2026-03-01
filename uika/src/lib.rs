// uika: User-facing library crate. Users depend on this and use `uika::entry!()`
// to generate the DLL entry points in their own cdylib crate.
//
//! ## Feature Flags
//!
//! | Feature              | Modules                                     |
//! |----------------------|---------------------------------------------|
//! | `core`               | CoreUObject types (UObject, UClass, ...)    |
//! | `engine`             | Engine types (AActor, UWorld, ...)           |
//! | `physics-core`       | PhysicsCore types                           |
//! | `input`              | EnhancedInput types                         |
//! | `slate`              | Slate UI types                              |
//! | `umg`                | UMG (Widget) types                          |
//! | `niagara`            | Niagara particle system types               |
//! | `gameplay-abilities` | Gameplay Ability System types                |
//! | `level-sequence`     | Level Sequence / Sequencer types            |
//! | `cinematic`          | Cinematic camera types                      |
//! | `movie`              | Movie scene types                           |

// Re-exports for proc macro path resolution and user access.
pub use uika_ffi as ffi;
pub use uika_runtime as runtime;
pub use uika_ue_flags as ue_flags;
pub use uika_bindings as bindings;
pub use uika_macros::{uclass, uclass_impl};

// For proc macro generated inventory::submit! invocations.
#[doc(hidden)]
pub extern crate inventory as __inventory;

pub mod prelude;

// Re-export glam for convenience.
pub use glam;

// ---------------------------------------------------------------------------
// Callbacks (shared between init and entry! macro)
// ---------------------------------------------------------------------------

extern "C" fn real_drop_rust_instance(
    handle: ffi::UObjectHandle,
    type_id: u64,
    _rust_data: *mut u8,
) {
    runtime::ffi_boundary((), || {
        runtime::reify_registry::drop_instance(handle, type_id);
    });
}

extern "C" fn real_invoke_rust_function(
    callback_id: u64,
    obj: ffi::UObjectHandle,
    params: *mut u8,
) {
    runtime::ffi_boundary((), || {
        // On native, NativePtr = *mut u8 so this is identity.
        // On wasm32, this path isn't used (wasm entry points handle conversion).
        runtime::reify_registry::invoke_function(callback_id, obj, params as runtime::ffi_dispatch::NativePtr);
    });
}

extern "C" fn real_invoke_delegate_callback(callback_id: u64, params: *mut u8) {
    runtime::ffi_boundary((), || {
        runtime::delegate_registry::invoke(callback_id, params as runtime::ffi_dispatch::NativePtr);
    });
}

extern "C" fn real_construct_rust_instance(
    obj: ffi::UObjectHandle,
    type_id: u64,
    _is_cdo: bool,
) {
    runtime::ffi_boundary((), || {
        runtime::reify_registry::construct_instance(obj, type_id);
    });
}

extern "C" fn real_on_shutdown() {
    runtime::ffi_boundary((), || {
        runtime::reify_registry::clear_all();
        runtime::delegate_registry::clear_all();
        runtime::pinned::clear_all();
    });
}

extern "C" fn real_notify_pinned_destroyed(handle: ffi::UObjectHandle) {
    runtime::ffi_boundary((), || {
        runtime::pinned::notify_pinned_destroyed(handle);
    });
}

#[doc(hidden)]
pub static __CALLBACKS: ffi::UikaRustCallbacks = ffi::UikaRustCallbacks {
    drop_rust_instance: real_drop_rust_instance,
    invoke_rust_function: real_invoke_rust_function,
    invoke_delegate_callback: real_invoke_delegate_callback,
    on_shutdown: real_on_shutdown,
    construct_rust_instance: real_construct_rust_instance,
    notify_pinned_destroyed: real_notify_pinned_destroyed,
};

// ---------------------------------------------------------------------------
// Init / Shutdown (called from entry!() generated code)
// ---------------------------------------------------------------------------

/// Initialize the Uika runtime. Called by the `entry!()` generated `uika_init`.
///
/// Stores the API table, registers all reified classes, and returns the
/// callback table pointer. Returns null on failure.
///
/// When the `wasm-host` feature is enabled, this delegates to `uika-wasm-host`
/// which loads `game.wasm` and forwards all callbacks into the WASM guest.
pub fn init(api_table: *const ffi::UikaApiTable) -> *const ffi::UikaRustCallbacks {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if api_table.is_null() {
            return std::ptr::null();
        }

        // Delegate API table storage to uika-runtime.
        runtime::init_api(api_table);

        #[cfg(feature = "wasm-host")]
        {
            // WASM host mode: load game.wasm and forward all callbacks into WASM.
            uika_wasm_host::init(api_table)
        }

        #[cfg(not(feature = "wasm-host"))]
        {
            // Native mode: run game code directly in this DLL.
            log_greeting();
            register_all_classes();
            &__CALLBACKS as *const ffi::UikaRustCallbacks
        }
    }))
    .unwrap_or(std::ptr::null())
}

/// Build a greeting string listing all compiled feature flags.
fn build_feature_greeting(prefix: &str) -> String {
    macro_rules! collect_features {
        ($s:expr, $($feat:literal),+ $(,)?) => {{
            $(
                #[cfg(feature = $feat)]
                $s.push_str(concat!(" ", $feat));
            )+
        }};
    }
    let mut s = format!("[Uika] {} (features:", prefix);
    collect_features!(s,
        "core", "engine", "physics-core", "input", "slate", "umg",
        "niagara", "gameplay-abilities", "level-sequence", "cinematic", "movie"
    );
    s.push(')');
    s
}

/// Log the Uika greeting message with compiled feature list.
#[cfg_attr(feature = "wasm-host", allow(dead_code))]
fn log_greeting() {
    let msg = build_feature_greeting("Rust side initialized");
    let bytes = msg.as_bytes();
    unsafe {
        runtime::ffi_dispatch::logging_log(0, bytes.as_ptr(), bytes.len() as u32);
    }
}

/// Register all Rust-defined UE classes via inventory auto-registration.
#[cfg_attr(feature = "wasm-host", allow(dead_code))]
fn register_all_classes() {
    runtime::reify_registry::register_all_from_inventory();
}

/// Shut down the Uika runtime. Called by the `entry!()` generated `uika_shutdown`.
pub fn shutdown() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[cfg(feature = "wasm-host")]
        {
            uika_wasm_host::shutdown();
        }
        #[cfg(not(feature = "wasm-host"))]
        {
            (__CALLBACKS.on_shutdown)();
        }
    }));
}

/// Initialize the Uika runtime for WASM guests. Called by `entry!()` wasm32 variant.
///
/// On wasm32, there is no API table — ffi_dispatch uses WASM imports directly.
/// This just logs the greeting and registers all reified classes.
#[cfg(target_arch = "wasm32")]
pub fn wasm_init() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let msg = build_feature_greeting("WASM guest initialized");
        let bytes = msg.as_bytes();
        unsafe {
            runtime::ffi_dispatch::logging_log(0, bytes.as_ptr(), bytes.len() as u32);
        }

        // Register all reified classes.
        register_all_classes();
    }));
}

/// Reload the WASM module from disk without swapping the DLL.
///
/// When the `wasm-host` feature is enabled, this shuts down the current
/// WASM instance and re-initializes from the latest `game.wasm` on disk.
/// Returns `true` on success, `false` otherwise.
pub fn reload_wasm() -> bool {
    #[cfg(feature = "wasm-host")]
    {
        uika_wasm_host::reload()
    }
    #[cfg(not(feature = "wasm-host"))]
    {
        false
    }
}

/// Generates DLL/WASM exports for the Uika runtime entry points.
///
/// Place this at the top of your cdylib crate's `lib.rs`:
/// ```ignore
/// uika::entry!();
/// ```
///
/// On native: generates `uika_init` / `uika_shutdown` DLL exports.
/// On wasm32: generates WASM exports for init, shutdown, and callback forwarding.
#[macro_export]
macro_rules! entry {
    () => {
        #[cfg(not(target_arch = "wasm32"))]
        mod __uika_native_entry {
            #[unsafe(no_mangle)]
            pub extern "C" fn uika_init(
                api_table: *const $crate::ffi::UikaApiTable,
            ) -> *const $crate::ffi::UikaRustCallbacks {
                $crate::init(api_table)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_shutdown() {
                $crate::shutdown()
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_reload_wasm() -> bool {
                $crate::reload_wasm()
            }
        }

        #[cfg(target_arch = "wasm32")]
        mod __uika_wasm_entry {
            #[unsafe(no_mangle)]
            pub extern "C" fn uika_wasm_init() {
                $crate::wasm_init();
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_on_shutdown() {
                $crate::runtime::ffi_boundary((), || {
                    $crate::runtime::reify_registry::clear_all();
                    $crate::runtime::delegate_registry::clear_all();
                    $crate::runtime::pinned::clear_all();
                });
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_invoke_delegate(callback_id: i64, params: i64) {
                $crate::runtime::ffi_boundary((), || {
                    $crate::runtime::delegate_registry::invoke(callback_id as u64, params as u64);
                });
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_invoke_function(callback_id: i64, obj: i64, params: i64) {
                $crate::runtime::ffi_boundary((), || {
                    let h = $crate::ffi::UObjectHandle::from_addr(obj as u64);
                    $crate::runtime::reify_registry::invoke_function(callback_id as u64, h, params as u64);
                });
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_construct_instance(obj: i64, type_id: i64, is_cdo: i32) {
                $crate::runtime::ffi_boundary((), || {
                    let h = $crate::ffi::UObjectHandle::from_addr(obj as u64);
                    $crate::runtime::reify_registry::construct_instance(h, type_id as u64);
                });
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_drop_instance(obj: i64, type_id: i64) {
                $crate::runtime::ffi_boundary((), || {
                    let h = $crate::ffi::UObjectHandle::from_addr(obj as u64);
                    $crate::runtime::reify_registry::drop_instance(h, type_id as u64);
                });
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn uika_notify_pinned_destroyed(obj: i64) {
                $crate::runtime::ffi_boundary((), || {
                    let h = $crate::ffi::UObjectHandle::from_addr(obj as u64);
                    $crate::runtime::pinned::notify_pinned_destroyed(h);
                });
            }
        }
    };
}
