use std::ffi::c_void;

/// Opaque handle to a UObject. Rust never dereferences — it is a C++ side identifier.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UObjectHandle(pub *mut c_void);

/// Opaque handle to a UClass.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UClassHandle(pub *mut c_void);

/// Opaque handle to an FProperty.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FPropertyHandle(pub *mut c_void);

/// Opaque handle to a UFunction. Only used for reify (Phase 9);
/// normal function calls go through func_table.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UFunctionHandle(pub *mut c_void);

/// Opaque handle to a UScriptStruct.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UStructHandle(pub *mut c_void);

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
