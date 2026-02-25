// TWeakObjectPtr<T>: typed weak reference to a UObject.
// Does not prevent garbage collection. Can be resolved to UObjectRef<T>
// if the object is still alive.

use std::marker::PhantomData;

use uika_ffi::{FWeakObjectHandle, UObjectHandle};

use crate::api::api;
use crate::object_ref::UObjectRef;
use crate::traits::UeClass;

/// A typed weak reference to a UObject.
///
/// Unlike `UObjectRef<T>`, a weak pointer uses UE's internal weak reference
/// system (ObjectIndex + SerialNumber) which can reliably detect when an
/// object has been garbage collected.
///
/// Use `get()` to attempt to resolve to a strong `UObjectRef<T>`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TWeakObjectPtr<T: UeClass> {
    handle: FWeakObjectHandle,
    _marker: PhantomData<*const T>,
}

unsafe impl<T: UeClass> Send for TWeakObjectPtr<T> {}

impl<T: UeClass> TWeakObjectPtr<T> {
    /// Create a weak pointer from a strong UObjectRef.
    pub fn from_ref(obj: &UObjectRef<T>) -> Self {
        let handle = unsafe { ((*api().core).make_weak)(obj.raw()) };
        TWeakObjectPtr {
            handle,
            _marker: PhantomData,
        }
    }

    /// Attempt to resolve to a strong reference. Returns `None` if the
    /// object has been garbage collected.
    pub fn get(&self) -> Option<UObjectRef<T>> {
        let obj = unsafe { ((*api().core).resolve_weak)(self.handle) };
        if obj == (UObjectHandle(std::ptr::null_mut())) {
            None
        } else {
            Some(unsafe { UObjectRef::from_raw(obj) })
        }
    }

    /// Check if the referenced object is still alive.
    pub fn is_valid(&self) -> bool {
        unsafe { ((*api().core).is_weak_valid)(self.handle) }
    }

    /// Get the underlying FFI handle.
    #[inline]
    pub fn handle(&self) -> FWeakObjectHandle {
        self.handle
    }
}

impl<T: UeClass> Default for TWeakObjectPtr<T> {
    fn default() -> Self {
        TWeakObjectPtr {
            handle: FWeakObjectHandle::default(),
            _marker: PhantomData,
        }
    }
}
