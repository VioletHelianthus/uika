// Logging bridge to UE_LOG.

/// Log level constants for the `ulog!` macro.
pub const LOG_DISPLAY: u8 = 0;
pub const LOG_WARNING: u8 = 1;
pub const LOG_ERROR: u8 = 2;

/// Log a message through UE_LOG.
///
/// Usage:
/// ```ignore
/// ulog!(LOG_DISPLAY, "Actor {} has {} health", name, hp);
/// ulog!(LOG_WARNING, "something suspicious");
/// ulog!(LOG_ERROR, "fatal: {err}");
/// ```
///
/// Level constants: `LOG_DISPLAY` (0), `LOG_WARNING` (1), `LOG_ERROR` (2).
#[macro_export]
macro_rules! ulog {
    ($level:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let bytes = msg.as_bytes();
        // SAFETY: api() is initialized before any Rust code can run, and the
        // logging sub-table pointer is always valid after init.
        unsafe {
            ((*$crate::api().logging).log)($level, bytes.as_ptr(), bytes.len() as u32);
        }
    }};
}
