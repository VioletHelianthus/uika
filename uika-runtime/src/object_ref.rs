// UObjectRef<T>: lightweight 8-byte Copy handle to a UObject.
//
// Does NOT prevent garbage collection — the referenced object may become
// invalid at any time between GC sweeps. Use `Pinned<T>` when you need
// to guarantee liveness.

use std::marker::PhantomData;
use std::ops::Deref;

use uika_ffi::{UClassHandle, UObjectHandle};

use crate::api::api;
use crate::error::{check_ffi, UikaError, UikaResult};
use crate::pinned::Pinned;
use crate::traits::{HasParent, UeClass, UeHandle, ValidHandle};

/// A typed, non-owning reference to a UObject.
///
/// - `Copy` + `Send` — can be freely cloned and sent across threads.
/// - `!Sync` — must only be *used* on the game thread.
/// - Does not prevent garbage collection; call [`is_valid`](Self::is_valid)
///   before use, or upgrade to [`Pinned<T>`] via [`pin`](Self::pin).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct UObjectRef<T: UeClass> {
    handle: UObjectHandle,
    _marker: PhantomData<*const T>, // *const T makes it !Sync
}

// Send: handles are raw identifiers safe to move between threads.
// !Sync: enforced by PhantomData<*const T> — no shared references across threads.
unsafe impl<T: UeClass> Send for UObjectRef<T> {}

impl<T: UeClass> UObjectRef<T> {
    /// Create from a raw FFI handle.
    ///
    /// # Safety
    /// The caller must ensure the handle points to an object whose UClass
    /// is `T` or a subclass of `T`.
    #[inline]
    pub unsafe fn from_raw(handle: UObjectHandle) -> Self {
        UObjectRef {
            handle,
            _marker: PhantomData,
        }
    }

    /// Get the underlying raw handle.
    #[inline]
    pub fn raw(&self) -> UObjectHandle {
        self.handle
    }

    /// Check whether the underlying UObject is still alive.
    #[inline]
    pub fn is_valid(&self) -> bool {
        unsafe { ((*api().core).is_valid)(self.handle) }
    }

    /// Validate that the object is still alive, returning a `Checked<T>`
    /// handle that provides infallible access to extension trait methods.
    #[inline]
    pub fn checked(&self) -> UikaResult<Checked<T>> {
        if self.is_valid() {
            Ok(Checked {
                handle: self.handle,
                _marker: PhantomData,
            })
        } else {
            Err(UikaError::ObjectDestroyed)
        }
    }

    /// Cast to a different UClass type. Fails if the object is destroyed
    /// or is not an instance of `U`.
    pub fn cast<U: UeClass>(self) -> UikaResult<UObjectRef<U>> {
        let h = self.checked()?.raw();
        let target = U::static_class();
        if unsafe { ((*api().core).is_a)(h, target) } {
            Ok(UObjectRef {
                handle: self.handle,
                _marker: PhantomData,
            })
        } else {
            Err(UikaError::InvalidCast)
        }
    }

    /// Upgrade to a `Pinned<T>`, adding a GC root to keep the object alive.
    pub fn pin(self) -> UikaResult<Pinned<T>> {
        Pinned::new(self)
    }

    /// Get the object's FName as a String.
    pub fn get_name(&self) -> UikaResult<String> {
        let h = self.checked()?.raw();
        // Stack buffer — 256 bytes is enough for virtually all UObject names.
        let mut buf = [0u8; 256];
        let mut out_len: u32 = 0;
        let code = unsafe {
            ((*api().core).get_name)(h, buf.as_mut_ptr(), buf.len() as u32, &mut out_len)
        };
        check_ffi(code)?;
        // C++ writes valid UTF-8 (converted from TCHAR).
        std::str::from_utf8(&buf[..out_len as usize])
            .map(|s| s.to_owned())
            .map_err(|_| UikaError::Internal("name is not valid UTF-8".into()))
    }

    /// Get the object's UClass handle.
    pub fn get_class(&self) -> UikaResult<UClassHandle> {
        let h = self.checked()?.raw();
        Ok(unsafe { ((*api().core).get_class)(h) })
    }

    /// Get the object's Outer.
    pub fn get_outer(&self) -> UikaResult<UObjectHandle> {
        let h = self.checked()?.raw();
        Ok(unsafe { ((*api().core).get_outer)(h) })
    }

    /// Check whether this object is an instance of `U` (or a subclass of `U`).
    /// Returns `false` if the object has been destroyed.
    #[inline]
    pub fn is_a<U: UeClass>(&self) -> bool {
        self.is_valid() && unsafe { ((*api().core).is_a)(self.handle, U::static_class()) }
    }
}

impl<T: HasParent> UObjectRef<T> {
    /// Infallible upcast to the parent class. Zero-cost (same handle).
    #[inline]
    pub fn upcast(self) -> UObjectRef<T::Parent> {
        unsafe { UObjectRef::from_raw(self.handle) }
    }
}

/// Blanket Deref: `UObjectRef<Child>` auto-derefs to `UObjectRef<Parent>`.
/// Safe because `UObjectRef<T>` is `#[repr(transparent)]` over `UObjectHandle`.
impl<T: HasParent> Deref for UObjectRef<T> {
    type Target = UObjectRef<T::Parent>;
    #[inline]
    fn deref(&self) -> &UObjectRef<T::Parent> {
        unsafe { &*(self as *const _ as *const UObjectRef<T::Parent>) }
    }
}

impl<T: UeClass> UeHandle for UObjectRef<T> {
    #[inline]
    fn checked_handle(&self) -> UikaResult<UObjectHandle> {
        self.checked().map(|c| c.raw())
    }

    #[inline]
    fn raw_handle(&self) -> UObjectHandle {
        self.raw()
    }
}

impl<T: UeClass> std::fmt::Debug for UObjectRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UObjectRef")
            .field("handle", &self.handle)
            .field("valid", &self.is_valid())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Checked<T>
// ---------------------------------------------------------------------------

/// A pre-validated handle to a UObject. Proves that the object was alive
/// at the time of validation. Used as the receiver for codegen extension
/// trait methods, which can then skip per-call validity checks.
///
/// Obtain via [`UObjectRef::checked()`] or [`Pinned::as_checked()`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Checked<T: UeClass> {
    handle: UObjectHandle,
    _marker: PhantomData<*const T>,
}

unsafe impl<T: UeClass> Send for Checked<T> {}

impl<T: UeClass> Checked<T> {
    /// Create a `Checked` handle without validation.
    /// Used internally by `Pinned::as_checked()`.
    #[inline]
    pub(crate) fn new_unchecked(handle: UObjectHandle) -> Self {
        Checked {
            handle,
            _marker: PhantomData,
        }
    }

    /// Get the underlying raw handle.
    #[inline]
    pub fn raw(&self) -> UObjectHandle {
        self.handle
    }

    /// Downgrade back to a `UObjectRef<T>`.
    #[inline]
    pub fn as_ref(&self) -> UObjectRef<T> {
        unsafe { UObjectRef::from_raw(self.handle) }
    }

    /// Check whether this object is an instance of `U` (or a subclass of `U`).
    /// No validity check needed — already validated at `Checked` construction.
    #[inline]
    pub fn is_a<U: UeClass>(&self) -> bool {
        unsafe { ((*api().core).is_a)(self.handle, U::static_class()) }
    }

    /// Cast to a different UClass type. Fails if not an instance of `U`.
    pub fn cast<U: UeClass>(self) -> UikaResult<Checked<U>> {
        if self.is_a::<U>() {
            Ok(Checked::new_unchecked(self.handle))
        } else {
            Err(UikaError::InvalidCast)
        }
    }
}

impl<T: HasParent> Checked<T> {
    /// Infallible upcast to the parent class. Zero-cost (same handle).
    #[inline]
    pub fn upcast(self) -> Checked<T::Parent> {
        Checked::new_unchecked(self.handle)
    }
}

/// Blanket Deref: `Checked<Child>` auto-derefs to `Checked<Parent>`.
/// Safe because `Checked<T>` is `#[repr(transparent)]` over `UObjectHandle`.
impl<T: HasParent> Deref for Checked<T> {
    type Target = Checked<T::Parent>;
    #[inline]
    fn deref(&self) -> &Checked<T::Parent> {
        unsafe { &*(self as *const _ as *const Checked<T::Parent>) }
    }
}

impl<T: UeClass> ValidHandle for Checked<T> {
    #[inline]
    fn handle(&self) -> UObjectHandle {
        self.handle
    }
}

impl<T: UeClass> std::fmt::Debug for Checked<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checked")
            .field("handle", &self.handle)
            .finish()
    }
}
