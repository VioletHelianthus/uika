// Global API table storage. Initialized once during DLL load, then read-only.

use std::sync::OnceLock;

use uika_ffi::UikaApiTable;

/// Wrapper so a raw pointer can live inside OnceLock (which requires Send+Sync).
/// SAFETY: The API table is created by C++ before uika_init and lives for the
/// entire DLL lifetime. Access is read-only after init.
struct ApiRef(*const UikaApiTable);
unsafe impl Send for ApiRef {}
unsafe impl Sync for ApiRef {}

static API: OnceLock<ApiRef> = OnceLock::new();

/// Store the API table pointer. Called once by `uika_init`.
/// Panics if called more than once.
pub fn init_api(table: *const UikaApiTable) {
    assert!(!table.is_null(), "init_api called with null pointer");
    if API.set(ApiRef(table)).is_err() {
        panic!("init_api called more than once");
    }
}

/// Access the global API table. Panics if called before `init_api`.
#[inline(always)]
pub fn api() -> &'static UikaApiTable {
    // SAFETY: The pointer was validated non-null in init_api, and the C++ side
    // guarantees the table outlives the DLL.
    unsafe { &*API.get().expect("uika API not initialized").0 }
}

/// Returns true if the API table has been initialized.
/// On wasm32, always returns true (imports are always available).
#[inline]
pub fn is_api_initialized() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    { API.get().is_some() }
    #[cfg(target_arch = "wasm32")]
    { true }
}
