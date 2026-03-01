//! UikaRustCallbacks: full forwarding from C++ → WASM exports.
//!
//! Each callback wraps the WASM call in `catch_unwind` to prevent panics from
//! unwinding through the `extern "C"` boundary (which is UB).

use std::panic::{catch_unwind, AssertUnwindSafe};

use uika_ffi::callbacks::UikaRustCallbacks;
use uika_ffi::handles::UObjectHandle;

use crate::with_host;

// ---------------------------------------------------------------------------
// Helper: catch panics + log
// ---------------------------------------------------------------------------

fn safe_callback(name: &str, f: impl FnOnce()) {
    let result = catch_unwind(AssertUnwindSafe(f));
    if let Err(e) = result {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        crate::ue_log(2, &format!("[WasmHost] PANIC in {name}: {msg}"));
    }
}

// ---------------------------------------------------------------------------
// Callback implementations
// ---------------------------------------------------------------------------

extern "C" fn drop_rust_instance(handle: UObjectHandle, type_id: u64, _rust_data: *mut u8) {
    let h = handle;
    safe_callback("drop_rust_instance", move || {
        with_host(|host| {
            let obj = h.0 as usize as i64;
            if let Err(e) = host.drop_instance(obj, type_id as i64) {
                crate::ue_log(2, &format!("[WasmHost] drop_instance error: {e}"));
            }
        });
    });
}

extern "C" fn invoke_rust_function(callback_id: u64, obj: UObjectHandle, params: *mut u8) {
    let o = obj;
    let p = params;
    safe_callback("invoke_rust_function", move || {
        with_host(|host| {
            let obj_i64 = o.0 as usize as i64;
            let params_i64 = p as usize as i64;
            if let Err(e) = host.invoke_function(callback_id, obj_i64, params_i64) {
                crate::ue_log(2, &format!("[WasmHost] invoke_function error: {e}"));
            }
        });
    });
}

extern "C" fn invoke_delegate_callback(callback_id: u64, params: *mut u8) {
    let p = params;
    safe_callback("invoke_delegate_callback", move || {
        with_host(|host| {
            let params_i64 = p as usize as i64;
            if let Err(e) = host.invoke_delegate(callback_id, params_i64) {
                crate::ue_log(2, &format!("[WasmHost] invoke_delegate error: {e}"));
            }
        });
    });
}

extern "C" fn on_shutdown() {
    safe_callback("on_shutdown", || {
        with_host(|host| {
            if let Err(e) = host.on_shutdown() {
                crate::ue_log(2, &format!("[WasmHost] on_shutdown error: {e}"));
            }
        });
    });
}

extern "C" fn construct_rust_instance(obj: UObjectHandle, type_id: u64, is_cdo: bool) {
    let o = obj;
    safe_callback("construct_rust_instance", move || {
        with_host(|host| {
            let obj_i64 = o.0 as usize as i64;
            if let Err(e) = host.construct_instance(obj_i64, type_id as i64, is_cdo as i32) {
                crate::ue_log(2, &format!("[WasmHost] construct_instance error: {e}"));
            }
        });
    });
}

extern "C" fn notify_pinned_destroyed(handle: UObjectHandle) {
    let h = handle;
    safe_callback("notify_pinned_destroyed", move || {
        with_host(|host| {
            let obj_i64 = h.0 as usize as i64;
            if let Err(e) = host.notify_pinned_destroyed(obj_i64) {
                crate::ue_log(2, &format!("[WasmHost] notify_pinned_destroyed error: {e}"));
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Public callback table
// ---------------------------------------------------------------------------

pub static CALLBACKS: UikaRustCallbacks = UikaRustCallbacks {
    drop_rust_instance,
    invoke_rust_function,
    invoke_delegate_callback,
    on_shutdown,
    construct_rust_instance,
    notify_pinned_destroyed,
};
