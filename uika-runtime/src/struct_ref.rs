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
pub struct UStructRef<T: UeStruct> {
    ptr: *mut u8,
    _marker: PhantomData<T>,
}

impl<T: UeStruct> UStructRef<T> {
    /// Create from a raw pointer to struct memory.
    ///
    /// # Safety
    /// The caller must ensure `ptr` points to valid memory containing a `T`.
    #[inline]
    pub unsafe fn from_raw(ptr: *mut u8) -> Self {
        UStructRef {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Get the raw pointer as a `UObjectHandle`.
    ///
    /// PropertyApi methods take `UObjectHandle` which is `*mut c_void` —
    /// this works for both UObject pointers and raw struct memory.
    #[inline]
    pub fn as_ptr(&self) -> UObjectHandle {
        UObjectHandle(self.ptr as *mut std::ffi::c_void)
    }
}

impl<T: UeStruct> std::fmt::Debug for UStructRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UStructRef")
            .field("ptr", &self.ptr)
            .finish()
    }
}
