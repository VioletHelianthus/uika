// FFI boundary guard: wraps Rust callbacks to catch panics before they
// cross the FFI boundary (which is undefined behavior).

use crate::api::is_api_initialized;

/// Execute `f` and catch any panic, returning `default` on failure.
///
/// All `extern "C"` functions called by C++ should wrap their body in this
/// guard. A panic that escapes across FFI is instant UB; this prevents that.
///
/// If the API table is initialized, the panic message is logged via UE_LOG.
pub fn ffi_boundary<F, R>(default: R, f: F) -> R
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(value) => value,
        Err(payload) => {
            // Best-effort logging. If the API isn't initialized yet, we can't
            // log through UE, so the panic is silently swallowed (still better
            // than UB).
            if is_api_initialized() {
                let msg = panic_message(&payload);
                let bytes = msg.as_bytes();
                unsafe {
                    let api = crate::api::api();
                    ((*api.logging).log)(2, bytes.as_ptr(), bytes.len() as u32);
                }
            }
            default
        }
    }
}

/// Extract a human-readable message from a panic payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("[Uika] Rust panic: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("[Uika] Rust panic: {s}")
    } else {
        "[Uika] Rust panic (unknown payload)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_boundary_returns_value_on_success() {
        let result = ffi_boundary(0i32, || 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn ffi_boundary_returns_default_on_panic() {
        let result = ffi_boundary(-1i32, || {
            panic!("test panic");
        });
        assert_eq!(result, -1);
    }

    #[test]
    fn ffi_boundary_returns_default_on_string_panic() {
        let result = ffi_boundary(false, || -> bool {
            panic!("{}", "formatted panic");
        });
        assert!(!result);
    }
}
