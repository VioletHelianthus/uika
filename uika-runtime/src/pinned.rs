// Pinned<T>: RAII GC root that keeps a UObject alive until dropped.
//
// Construction calls add_gc_root + register_pinned; Drop calls unregister_pinned
// + remove_gc_root. The GC root prevents garbage collection, while the pinned
// registration enables fast alive-flag checking via a local AtomicBool instead
// of an FFI is_valid call on every method invocation.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use uika_ffi::UObjectHandle;

use crate::api::api;
use crate::error::{UikaError, UikaResult};
use crate::object_ref::{Checked, UObjectRef};
use crate::traits::{UeClass, UeHandle, ValidHandle};

// ---------------------------------------------------------------------------
// Alive registry — maps UObject pointer → alive flag for fast checked_handle
// ---------------------------------------------------------------------------

fn alive_registry() -> &'static Mutex<HashMap<usize, Arc<AtomicBool>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<AtomicBool>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Called from C++ (via FUikaRustCallbacks) when a pinned object is destroyed
/// by DestroyActor, level unload, PIE end, etc. Sets the alive flag to false
/// so subsequent `checked_handle()` calls return `Err(ObjectDestroyed)`.
pub fn notify_pinned_destroyed(handle: UObjectHandle) {
    if let Ok(registry) = alive_registry().lock() {
        if let Some(flag) = registry.get(&(handle.0 as usize)) {
            flag.store(false, Ordering::Relaxed);
        }
    }
}

/// Clear all alive flags. Called during on_shutdown (hot reload / DLL unload).
pub fn clear_all() {
    if let Ok(mut registry) = alive_registry().lock() {
        registry.clear();
    }
}

// ---------------------------------------------------------------------------
// Pinned<T>
// ---------------------------------------------------------------------------

/// An owning GC root for a UObject.
///
/// - `!Copy`, `!Clone` — unique ownership of the GC root.
/// - `Send` — can be moved across threads.
/// - `!Sync` — must only be *used* on the game thread.
/// - `Drop` removes the GC root and unregisters from destroy notification.
///
/// Method calls on `Pinned<T>` use a local alive flag (~1-3 cycles) instead
/// of an FFI `is_valid` call (~15-30 cycles) for validity checking.
pub struct Pinned<T: UeClass> {
    handle: UObjectHandle,
    alive: Arc<AtomicBool>,
    _marker: PhantomData<*const T>, // !Sync
}

unsafe impl<T: UeClass> Send for Pinned<T> {}

impl<T: UeClass> Pinned<T> {
    /// Pin an object by adding a GC root and registering for destroy notification.
    /// Fails if the object is already destroyed.
    pub fn new(obj: UObjectRef<T>) -> UikaResult<Self> {
        if !obj.is_valid() {
            return Err(UikaError::ObjectDestroyed);
        }
        let alive = Arc::new(AtomicBool::new(true));
        // Register in alive registry (for C++ destroy notification → Rust alive flag).
        alive_registry().lock().unwrap()
            .insert(obj.raw().0 as usize, alive.clone());
        // GC root + destroy notification registration.
        unsafe {
            ((*api().lifecycle).add_gc_root)(obj.raw());
            ((*api().lifecycle).register_pinned)(obj.raw());
        }
        Ok(Pinned {
            handle: obj.raw(),
            alive,
            _marker: PhantomData,
        })
    }

    /// Check whether the pinned object is still alive (local memory read).
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Get the underlying raw handle. Guaranteed valid while this `Pinned`
    /// exists and `is_alive()` returns true.
    #[inline]
    pub fn handle(&self) -> UObjectHandle {
        self.handle
    }

    /// Borrow as a lightweight `UObjectRef`. The returned ref is valid as long
    /// as this `Pinned` is alive.
    #[inline]
    pub fn as_ref(&self) -> UObjectRef<T> {
        // SAFETY: The GC root guarantees the object is alive, and we know
        // the type is correct because it was validated at construction.
        unsafe { UObjectRef::from_raw(self.handle) }
    }

    /// Create a `Checked<T>` handle from this pinned reference.
    /// Valid as long as `is_alive()` returns true (debug-asserted).
    #[inline]
    pub fn as_checked(&self) -> Checked<T> {
        debug_assert!(self.is_alive(), "Pinned object has been destroyed");
        Checked::new_unchecked(self.handle)
    }
}

impl<T: UeClass> Drop for Pinned<T> {
    fn drop(&mut self) {
        // Remove from alive registry.
        alive_registry().lock().unwrap().remove(&(self.handle.0 as usize));
        // Unregister from C++ destroy notification, then remove GC root.
        unsafe {
            ((*api().lifecycle).unregister_pinned)(self.handle);
            ((*api().lifecycle).remove_gc_root)(self.handle);
        }
    }
}

impl<T: UeClass> ValidHandle for Pinned<T> {
    #[inline]
    fn handle(&self) -> UObjectHandle {
        debug_assert!(self.is_alive(), "Pinned object has been destroyed");
        self.handle
    }
}

impl<T: UeClass> UeHandle for Pinned<T> {
    #[inline]
    fn checked_handle(&self) -> UikaResult<UObjectHandle> {
        // Local memory read (~1-3 cycles) instead of FFI is_valid (~15-30 cycles).
        if self.alive.load(Ordering::Relaxed) {
            Ok(self.handle)
        } else {
            Err(UikaError::ObjectDestroyed)
        }
    }

    #[inline]
    fn raw_handle(&self) -> UObjectHandle {
        self.handle
    }
}

impl<T: UeClass> std::fmt::Debug for Pinned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pinned")
            .field("handle", &self.handle)
            .field("alive", &self.is_alive())
            .finish()
    }
}
