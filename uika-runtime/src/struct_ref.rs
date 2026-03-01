// UStructRef<T>: lightweight typed wrapper for struct memory pointers.
//
// Unlike UObjectRef<T>, struct memory is not garbage-collected by UE.
// UStructRef is just a typed raw pointer to a struct instance in memory
// (e.g., inside a UObject property or a parameter buffer).

use std::marker::PhantomData;

use uika_ffi::UObjectHandle;

use crate::traits::UeStruct;

/// A typed, non-owning reference to a UE struct instance in memory.
///
/// This wraps a raw pointer to struct data (e.g., an FVector stored inside
/// a UObject property). The struct memory is managed by its container —
/// no validity check is needed (unlike UObjectRef).
///
/// PropertyApi methods accept `UObjectHandle` (`*mut c_void`) which works
/// for both UObject pointers and raw struct memory pointers.
///
/// On wasm32, struct memory lives in native (host) memory and the pointer
/// is stored as a `u64` to avoid truncation (wasm32 pointers are 32-bit).
pub struct UStructRef<T: UeStruct> {
    #[cfg(not(target_arch = "wasm32"))]
    ptr: *mut u8,
    #[cfg(target_arch = "wasm32")]
    native_ptr: u64,
    _marker: PhantomData<T>,
}

impl<T: UeStruct> UStructRef<T> {
    /// Create from a raw pointer to struct memory.
    ///
    /// # Safety
    /// The caller must ensure `ptr` points to valid memory containing a `T`.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub unsafe fn from_raw(ptr: *mut u8) -> Self {
        UStructRef {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Create from a native (host) memory pointer on wasm32.
    ///
    /// # Safety
    /// The caller must ensure `native_ptr` points to valid native memory containing a `T`.
    #[cfg(target_arch = "wasm32")]
    #[inline]
    pub unsafe fn from_native_ptr(native_ptr: u64) -> Self {
        UStructRef {
            native_ptr,
            _marker: PhantomData,
        }
    }

    /// Get the raw pointer as a `UObjectHandle`.
    ///
    /// PropertyApi methods take `UObjectHandle` which is `*mut c_void` —
    /// this works for both UObject pointers and raw struct memory.
    #[inline]
    pub fn as_ptr(&self) -> UObjectHandle {
        #[cfg(not(target_arch = "wasm32"))]
        { UObjectHandle(self.ptr as *mut std::ffi::c_void) }
        #[cfg(target_arch = "wasm32")]
        { UObjectHandle(self.native_ptr) }
    }
}

impl<T: UeStruct> std::fmt::Debug for UStructRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(not(target_arch = "wasm32"))]
        { f.debug_struct("UStructRef").field("ptr", &self.ptr).finish() }
        #[cfg(target_arch = "wasm32")]
        { f.debug_struct("UStructRef").field("native_ptr", &self.native_ptr).finish() }
    }
}
