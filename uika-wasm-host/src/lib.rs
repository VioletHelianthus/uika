//! uika-wasm-host: WASM host runtime for uika.
//!
//! Embeds wasmtime to load and run user game code compiled to wasm32.
//! Auto-generates host function bridges from `uika-ffi/api_table.rs`.

mod callbacks;
mod dll_path;
pub mod wasm_host;

use std::cell::UnsafeCell;
use std::panic::catch_unwind;
use std::ptr;

use uika_ffi::api_table::UikaApiTable;
use uika_ffi::callbacks::UikaRustCallbacks;

use wasm_host::WasmHost;

// ---------------------------------------------------------------------------
// Global state (game-thread only, no synchronization needed)
// ---------------------------------------------------------------------------

struct WasmHostCell(UnsafeCell<Option<WasmHost>>);
unsafe impl Sync for WasmHostCell {}

static WASM_HOST: WasmHostCell = WasmHostCell(UnsafeCell::new(None));

pub(crate) fn with_host<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut WasmHost) -> R,
{
    let host = unsafe { &mut *WASM_HOST.0.get() };
    host.as_mut().map(f)
}

// ---------------------------------------------------------------------------
// Logging helper (uses the API table directly)
// ---------------------------------------------------------------------------

static mut API_PTR: *const UikaApiTable = ptr::null();

/// Log a message through UE's logging system.
pub(crate) fn ue_log(level: u8, msg: &str) {
    unsafe {
        if !API_PTR.is_null() {
            let logging = &*(*API_PTR).logging;
            (logging.log)(level, msg.as_ptr(), msg.len() as u32);
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialize the WASM host. Called from `uika_init` when the `wasm-host` feature is active.
///
/// Returns a pointer to the callback table on success, null on failure.
pub fn init(api: *const UikaApiTable) -> *const UikaRustCallbacks {
    let result = catch_unwind(|| unsafe { do_init(api) });
    match result {
        Ok(Some(cb)) => cb,
        Ok(None) => ptr::null(),
        Err(_) => ptr::null(),
    }
}

/// Reload WASM module from disk. Shutdown + re-init with same API table.
pub fn reload() -> bool {
    let api = unsafe { API_PTR };
    if api.is_null() {
        return false;
    }
    shutdown();
    !init(api).is_null()
}

/// Shutdown the WASM host. Called from `uika_shutdown`.
pub fn shutdown() {
    // Call on_shutdown before dropping
    with_host(|host| {
        if let Err(e) = host.on_shutdown() {
            ue_log(2, &format!("[WasmHost] on_shutdown error: {e}"));
        }
    });
    unsafe { *WASM_HOST.0.get() = None };
    unsafe { API_PTR = ptr::null() };
}

// ---------------------------------------------------------------------------
// Internal initialization
// ---------------------------------------------------------------------------

unsafe fn do_init(api: *const UikaApiTable) -> Option<*const UikaRustCallbacks> {
    unsafe { API_PTR = api };

    // Locate game.wasm next to this DLL
    let wasm_path = match dll_path::find_wasm_file() {
        Some(p) => p,
        None => {
            ue_log(2, "[WasmHost] Failed to locate DLL directory");
            return None;
        }
    };

    ue_log(
        0,
        &format!("[WasmHost] Loading WASM from: {}", wasm_path.display()),
    );

    let wasm_bytes = match std::fs::read(&wasm_path) {
        Ok(b) => b,
        Err(e) => {
            ue_log(
                2,
                &format!("[WasmHost] Failed to read {}: {e}", wasm_path.display()),
            );
            return None;
        }
    };

    ue_log(
        0,
        &format!(
            "[WasmHost] Module loaded: game.wasm ({} bytes)",
            wasm_bytes.len()
        ),
    );

    // Create WasmHost
    let mut host = match WasmHost::new(api, &wasm_bytes) {
        Ok(h) => h,
        Err(e) => {
            ue_log(2, &format!("[WasmHost] Failed to create WasmHost: {e}"));
            return None;
        }
    };

    // Call WASM init
    if let Err(e) = host.call_init() {
        ue_log(2, &format!("[WasmHost] WASM init failed: {e}"));
        return None;
    }

    ue_log(0, "[WasmHost] WASM initialized successfully");

    // Store globally
    unsafe { *WASM_HOST.0.get() = Some(host) };

    Some(&callbacks::CALLBACKS as *const UikaRustCallbacks)
}
