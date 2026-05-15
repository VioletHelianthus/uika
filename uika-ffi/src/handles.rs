use std::ffi::c_void;

// ---------------------------------------------------------------------------
// Macro: define a pointer-based handle (opaque C++ side identifier).
// ---------------------------------------------------------------------------

macro_rules! define_ptr_handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name(pub *mut c_void);

        impl $name {
            pub fn is_null(&self) -> bool {
                self.0.is_null()
            }

            pub fn null() -> Self {
                Self(std::ptr::null_mut())
            }

            /// Create from a raw u64 address (platform-agnostic byte-level construction).
            pub fn from_addr(addr: u64) -> Self {
                Self(addr as usize as *mut c_void)
            }

            /// Convert to a raw u64 address (platform-agnostic byte-level extraction).
            pub fn to_addr(&self) -> u64 {
                self.0 as usize as u64
            }
        }
    };
}

define_ptr_handle! {
    /// Opaque handle to a UObject. Rust never dereferences — it is a C++ side identifier.
    UObjectHandle
}

define_ptr_handle! {
    /// Opaque handle to a UClass.
    UClassHandle
}

define_ptr_handle! {
    /// Opaque handle to an FProperty.
    FPropertyHandle
}

define_ptr_handle! {
    /// Opaque handle to a UFunction. Only used for reify (Phase 9);
    /// normal function calls go through func_table.
    UFunctionHandle
}

define_ptr_handle! {
    /// Opaque handle to a UScriptStruct.
    UStructHandle
}

/// FName stored as a raw 64-bit value (ComparisonIndex + Number).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FNameHandle(pub u64);

/// Weak object pointer: ObjectIndex + ObjectSerialNumber (matches UE FWeakObjectPtr layout).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FWeakObjectHandle {
    pub object_index: i32,
    pub object_serial_number: i32,
}

impl Default for FWeakObjectHandle {
    fn default() -> Self {
        FWeakObjectHandle {
            object_index: -1,
            object_serial_number: 0,
        }
    }
}

// Handles are raw FFI identifiers. They can be sent across threads
// (but must only be *used* on the game thread).
// Sync is needed for OnceLock caching in generated code.
unsafe impl Send for UObjectHandle {}
unsafe impl Sync for UObjectHandle {}
unsafe impl Send for UClassHandle {}
unsafe impl Sync for UClassHandle {}
unsafe impl Send for FPropertyHandle {}
unsafe impl Sync for FPropertyHandle {}
unsafe impl Send for UFunctionHandle {}
unsafe impl Sync for UFunctionHandle {}
unsafe impl Send for UStructHandle {}
unsafe impl Sync for UStructHandle {}
unsafe impl Send for FWeakObjectHandle {}
unsafe impl Sync for FWeakObjectHandle {}
