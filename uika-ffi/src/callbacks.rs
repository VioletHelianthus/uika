use crate::handles::UObjectHandle;

/// Callback table filled by Rust and returned to C++ from `uika_init`.
/// C++ calls into Rust through these function pointers.
#[repr(C)]
pub struct UikaRustCallbacks {
    /// Called when a Rust-defined UClass instance is destroyed (reify only).
    pub drop_rust_instance: extern "C" fn(
        handle: UObjectHandle,
        type_id: u64,
        rust_data: *mut u8,
    ),

    /// UE → Rust function call forwarding (for Rust-defined UFunctions).
    pub invoke_rust_function: extern "C" fn(
        callback_id: u64,
        obj: UObjectHandle,
        params: *mut u8,
    ),

    /// Delegate callback forwarding.
    pub invoke_delegate_callback: extern "C" fn(callback_id: u64, params: *mut u8),

    /// Shutdown notification — Rust should release all resources.
    pub on_shutdown: extern "C" fn(),

    /// Called by C++ when a reified class instance is constructed.
    pub construct_rust_instance: extern "C" fn(
        obj: UObjectHandle,
        type_id: u64,
        is_cdo: bool,
    ),

    /// Called by C++ when a Pinned object is destroyed (DestroyActor, level unload, etc.).
    pub notify_pinned_destroyed: extern "C" fn(handle: UObjectHandle),
}
