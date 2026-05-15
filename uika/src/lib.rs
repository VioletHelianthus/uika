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
        runtime::reify_registry::invoke_function(callback_id, obj, params);
    });
}

extern "C" fn real_invoke_delegate_callback(callback_id: u64, params: *mut u8) {
    runtime::ffi_boundary((), || {
        runtime::delegate_registry::invoke(callback_id, params);
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
pub fn init(api_table: *const ffi::UikaApiTable) -> *const ffi::UikaRustCallbacks {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if api_table.is_null() {
            return std::ptr::null();
        }

        // Delegate API table storage to uika-runtime.
        runtime::init_api(api_table);

        log_greeting();
        register_all_classes();
        &__CALLBACKS as *const ffi::UikaRustCallbacks
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
fn log_greeting() {
    let msg = build_feature_greeting("Rust side initialized");
    let bytes = msg.as_bytes();
    unsafe {
        runtime::ffi_dispatch::logging_log(0, bytes.as_ptr(), bytes.len() as u32);
    }
}

/// Register all Rust-defined UE classes via inventory auto-registration.
fn register_all_classes() {
    runtime::reify_registry::register_all_from_inventory();
}

/// Shut down the Uika runtime. Called by the `entry!()` generated `uika_shutdown`.
pub fn shutdown() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (__CALLBACKS.on_shutdown)();
    }));
}

/// Generates DLL exports for the Uika runtime entry points.
///
/// Place this at the top of your cdylib crate's `lib.rs`:
/// ```ignore
/// uika::entry!();
/// ```
#[macro_export]
macro_rules! entry {
    () => {
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
        }
    };
}
