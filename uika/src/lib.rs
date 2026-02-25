// uika: User-facing library crate. Users depend on this and use `uika::entry!()`
// to generate the DLL entry points in their own cdylib crate.

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

        // Log greeting with compiled feature list.
        let table = unsafe { &*api_table };
        if !table.logging.is_null() {
            let log = |msg: &[u8]| unsafe {
                ((*table.logging).log)(0, msg.as_ptr(), msg.len() as u32);
            };

            macro_rules! feature_str {
                ($($feat:literal),+ $(,)?) => {{
                    let mut s = String::from("[Uika] Rust side initialized (features:");
                    $(
                        #[cfg(feature = $feat)]
                        s.push_str(concat!(" ", $feat));
                    )+
                    s.push(')');
                    s
                }};
            }
            let msg = feature_str!(
                "core", "engine", "physics-core", "input", "slate", "umg",
                "niagara", "gameplay-abilities", "level-sequence", "cinematic", "movie"
            );
            log(msg.as_bytes());
        }

        // Register all reified classes.
        register_all_classes(table);

        &__CALLBACKS as *const ffi::UikaRustCallbacks
    }))
    .unwrap_or(std::ptr::null())
}

/// Register all Rust-defined UE classes via inventory auto-registration.
fn register_all_classes(table: &ffi::UikaApiTable) {
    if table.reify.is_null() {
        return;
    }
    runtime::reify_registry::register_all_from_inventory(table);
}

/// Shut down the Uika runtime. Called by the `entry!()` generated `uika_shutdown`.
pub fn shutdown() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (__CALLBACKS.on_shutdown)();
    }));
}

/// Generates `uika_init` / `uika_shutdown` DLL exports that delegate to
/// `uika::init()` and `uika::shutdown()`.
///
/// Place this at the top of your cdylib crate's `lib.rs`:
/// ```ignore
/// uika::entry!();
/// ```
#[macro_export]
macro_rules! entry {
    () => {
        #[no_mangle]
        pub extern "C" fn uika_init(
            api_table: *const $crate::ffi::UikaApiTable,
        ) -> *const $crate::ffi::UikaRustCallbacks {
            $crate::init(api_table)
        }

        #[no_mangle]
        pub extern "C" fn uika_shutdown() {
            $crate::shutdown()
        }
    };
}
