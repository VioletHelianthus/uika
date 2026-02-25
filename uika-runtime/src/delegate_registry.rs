// Delegate callback registry: maps callback IDs to Rust closures.
// When UE fires a delegate, the C++ proxy calls invoke_delegate_callback(id, params),
// which looks up and calls the registered closure.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use uika_ffi::{FPropertyHandle, UObjectHandle, UikaErrorCode};

use crate::error::{check_ffi, UikaResult};

type DelegateCallback = Option<Box<dyn FnMut(*mut u8) + Send>>;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static REGISTRY: OnceLock<Mutex<HashMap<u64, DelegateCallback>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<u64, DelegateCallback>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a closure and return its unique callback ID.
pub fn register_callback(f: impl FnMut(*mut u8) + Send + 'static) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    registry().lock().unwrap().insert(id, Some(Box::new(f)));
    id
}

/// Unregister a callback by its ID.
pub fn unregister_callback(id: u64) {
    registry().lock().unwrap().remove(&id);
}

/// Clear all callbacks and reset the ID counter.
/// Called during shutdown before DLL unload (enables hot reload).
pub fn clear_all() {
    if let Some(reg) = REGISTRY.get() {
        reg.lock().unwrap().clear();
    }
    NEXT_ID.store(1, Ordering::Relaxed);
}

/// Invoke a registered callback. Called from the FFI boundary.
///
/// Uses a take-execute-replace pattern to avoid holding the registry lock
/// during callback execution, which would deadlock if the callback
/// registers, unregisters, or invokes other delegates.
pub fn invoke(callback_id: u64, params: *mut u8) {
    // 1. Briefly lock, take the callback out (replace with None).
    let mut cb = {
        let mut reg = registry().lock().unwrap();
        reg.get_mut(&callback_id).and_then(|slot| slot.take())
    };

    // 2. Execute outside the lock — callback may freely access the registry.
    if let Some(ref mut f) = cb {
        f(params);
    }

    // 3. Put back only if the slot still exists and is None.
    //    If unregister_callback was called during execution, the entry was
    //    removed entirely, so get_mut returns None and we drop the callback.
    if let Some(f) = cb {
        let mut reg = registry().lock().unwrap();
        if let Some(slot) = reg.get_mut(&callback_id) {
            if slot.is_none() {
                *slot = Some(f);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DelegateBinding — RAII handle for delegate bindings
// ---------------------------------------------------------------------------

/// RAII handle that unbinds a delegate and unregisters the callback on drop.
pub struct DelegateBinding {
    callback_id: u64,
    owner: UObjectHandle,
    prop: FPropertyHandle,
    is_multicast: bool,
}

impl DelegateBinding {
    /// Create a new binding handle. Should only be called by generated code.
    pub fn new(
        callback_id: u64,
        owner: UObjectHandle,
        prop: FPropertyHandle,
        is_multicast: bool,
    ) -> Self {
        Self {
            callback_id,
            owner,
            prop,
            is_multicast,
        }
    }

    /// Get the callback ID.
    pub fn callback_id(&self) -> u64 {
        self.callback_id
    }

    /// Manually unbind without waiting for drop. Consumes self.
    pub fn unbind(self) {
        // Drop will handle the cleanup.
    }
}

impl Drop for DelegateBinding {
    fn drop(&mut self) {
        // Unregister from Rust registry.
        unregister_callback(self.callback_id);

        // Unbind on the C++ side.
        unsafe {
            let api = crate::api::api();
            if !api.delegate.is_null() {
                if self.is_multicast {
                    let _ = ((*api.delegate).remove_multicast)(
                        self.owner,
                        self.prop,
                        self.callback_id,
                    );
                } else {
                    let _ = ((*api.delegate).unbind_delegate)(self.owner, self.prop);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// High-level bind helpers (used by generated code)
// ---------------------------------------------------------------------------

/// Bind a Rust closure to a unicast delegate property.
pub fn bind_unicast(
    owner: UObjectHandle,
    prop: FPropertyHandle,
    callback: impl FnMut(*mut u8) + Send + 'static,
) -> UikaResult<DelegateBinding> {
    let id = register_callback(callback);
    let result = unsafe { ((*crate::api::api().delegate).bind_delegate)(owner, prop, id) };
    if result != UikaErrorCode::Ok {
        unregister_callback(id);
        check_ffi(result)?;
    }
    Ok(DelegateBinding::new(id, owner, prop, false))
}

/// Add a Rust closure to a multicast delegate property.
pub fn bind_multicast(
    owner: UObjectHandle,
    prop: FPropertyHandle,
    callback: impl FnMut(*mut u8) + Send + 'static,
) -> UikaResult<DelegateBinding> {
    let id = register_callback(callback);
    let result = unsafe { ((*crate::api::api().delegate).add_multicast)(owner, prop, id) };
    if result != UikaErrorCode::Ok {
        unregister_callback(id);
        check_ffi(result)?;
    }
    Ok(DelegateBinding::new(id, owner, prop, true))
}
