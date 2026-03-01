// Reify registry: manages Rust-side type info, function callbacks, and instance data
// for runtime-created UE classes.
//
// Three registries:
// 1. Type registry: maps type_id -> RustTypeInfo (constructor, destructor, name)
// 2. Function registry: maps callback_id -> Rust function closure
// 3. Instance data: maps UObject pointer -> allocated Rust data

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

// ---------------------------------------------------------------------------
// Inventory-based auto-registration
// ---------------------------------------------------------------------------

/// Submitted by `#[uclass]` — holds register + finalize fn pointers.
pub struct ClassRegistration {
    pub register: fn(),
    pub finalize: fn(),
}
inventory::collect!(ClassRegistration);

/// Submitted by `#[uclass_impl]` — holds register_functions fn pointer.
pub struct ClassFunctionRegistration {
    pub register_functions: fn(),
}
inventory::collect!(ClassFunctionRegistration);

/// Three-phase iteration: register all → register all functions → finalize all.
pub fn register_all_from_inventory() {
    let mut class_count = 0u32;
    for reg in inventory::iter::<ClassRegistration> {
        (reg.register)();
        class_count += 1;
    }
    let mut func_reg_count = 0u32;
    for freg in inventory::iter::<ClassFunctionRegistration> {
        (freg.register_functions)();
        func_reg_count += 1;
    }
    for reg in inventory::iter::<ClassRegistration> {
        (reg.finalize)();
    }

    // Log registration summary (helps diagnose hot-reload issues).
    let total_funcs = func_registry().read().unwrap().len();
    let msg = format!(
        "[Uika] register_all_from_inventory: {} classes, {} impl blocks, {} function callbacks",
        class_count, func_reg_count, total_funcs,
    );
    let bytes = msg.as_bytes();
    unsafe {
        crate::ffi_dispatch::logging_log(0, bytes.as_ptr(), bytes.len() as u32);
    }
}

use uika_ffi::UObjectHandle;

/// Information about a Rust type registered for reification.
pub struct RustTypeInfo {
    /// Human-readable type name (for debugging).
    pub name: &'static str,
    /// Allocate and return a default-initialized instance. The returned pointer
    /// must be freeable by `drop_fn`.
    pub construct_fn: fn() -> *mut u8,
    /// Drop and deallocate an instance previously created by `construct_fn`.
    pub drop_fn: unsafe fn(*mut u8),
}

use crate::ffi_dispatch::NativePtr;

// Type for reify function callbacks: (obj, rust_data, params)
// Uses Arc so we can clone the reference out of the registry and release
// the lock before invoking the callback (prevents deadlock if the callback
// makes FFI calls that re-enter Rust).
// `params` is NativePtr: `*mut u8` on native, `u64` on wasm32.
type ReifyFunctionCallback = Arc<dyn Fn(UObjectHandle, *mut u8, NativePtr) + Send + Sync>;

// ---------------------------------------------------------------------------
// Statics
// ---------------------------------------------------------------------------

static TYPE_REGISTRY: OnceLock<Mutex<HashMap<u64, RustTypeInfo>>> = OnceLock::new();
static FUNC_REGISTRY: OnceLock<RwLock<Vec<ReifyFunctionCallback>>> = OnceLock::new();
static INSTANCE_DATA: OnceLock<RwLock<HashMap<u64, InstanceEntry>>> = OnceLock::new();

struct InstanceEntry {
    data: *mut u8,
    type_id: u64,
}

// SAFETY: The raw pointer in InstanceEntry is only accessed on the game thread.
unsafe impl Send for InstanceEntry {}
unsafe impl Sync for InstanceEntry {}

fn type_registry() -> &'static Mutex<HashMap<u64, RustTypeInfo>> {
    TYPE_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn func_registry() -> &'static RwLock<Vec<ReifyFunctionCallback>> {
    FUNC_REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

fn instance_data() -> &'static RwLock<HashMap<u64, InstanceEntry>> {
    INSTANCE_DATA.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Convert a UObjectHandle to a u64 key for HashMap storage.
#[inline]
fn handle_to_key(h: UObjectHandle) -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    { h.0 as usize as u64 }
    #[cfg(target_arch = "wasm32")]
    { h.0 }
}

// ---------------------------------------------------------------------------
// Type registry
// ---------------------------------------------------------------------------

/// Register a Rust type for reification. Must be called before `create_class`.
pub fn register_type(type_id: u64, info: RustTypeInfo) {
    type_registry()
        .lock()
        .unwrap()
        .insert(type_id, info);
}

// ---------------------------------------------------------------------------
// Function registry
// ---------------------------------------------------------------------------

/// Register a Rust function callback and return its unique callback ID.
pub fn register_function<F>(f: F) -> u64
where
    F: Fn(UObjectHandle, *mut u8, NativePtr) + Send + Sync + 'static,
{
    let mut vec = func_registry().write().unwrap();
    let id = vec.len() as u64;
    vec.push(Arc::new(f));
    id
}

// ---------------------------------------------------------------------------
// Instance lifecycle
// ---------------------------------------------------------------------------

/// Construct a Rust instance for a newly created UObject.
/// Called from the C++ class constructor via `construct_rust_instance` callback.
pub fn construct_instance(obj: UObjectHandle, type_id: u64) {
    let types = type_registry().lock().unwrap();
    let Some(info) = types.get(&type_id) else {
        // Log warning — type not registered (might be a CDO before registration completes)
        if crate::api::is_api_initialized() {
            let msg = format!("[Uika] construct_instance: unknown type_id {type_id}");
            let bytes = msg.as_bytes();
            unsafe {
                crate::ffi_dispatch::logging_log(1, bytes.as_ptr(), bytes.len() as u32);
            }
        }
        return;
    };
    let data = (info.construct_fn)();
    drop(types); // Release lock before acquiring instance_data lock

    let key = handle_to_key(obj);
    instance_data()
        .write()
        .unwrap()
        .insert(key, InstanceEntry { data, type_id });
}

/// Drop and remove the Rust instance for a destroyed UObject.
/// Called from the C++ delete listener via `drop_rust_instance` callback.
pub fn drop_instance(obj: UObjectHandle, _type_id: u64) {
    let key = handle_to_key(obj);
    let entry = instance_data().write().unwrap().remove(&key);

    if let Some(entry) = entry {
        let types = type_registry().lock().unwrap();
        if let Some(info) = types.get(&entry.type_id) {
            unsafe {
                (info.drop_fn)(entry.data);
            }
        }
    }
}

/// Invoke a registered Rust function callback.
/// Called from the C++ thunk via `invoke_rust_function` callback.
pub fn invoke_function(callback_id: u64, obj: UObjectHandle, params: NativePtr) {
    let key = handle_to_key(obj);

    // Look up instance data for this object (read lock — non-exclusive).
    let rust_data = instance_data()
        .read()
        .unwrap()
        .get(&key)
        .map(|e| e.data)
        .unwrap_or(std::ptr::null_mut());

    // Clone the callback Arc out of the registry and release the read lock
    // BEFORE invoking the callback. This prevents deadlocks if the callback
    // makes FFI calls that re-enter Rust.
    let func = {
        let vec = func_registry().read().unwrap();
        vec.get(callback_id as usize).cloned()
    };

    if let Some(func) = func {
        func(obj, rust_data, params);
    } else if crate::api::is_api_initialized() {
        let vec_len = func_registry().read().unwrap().len();
        let msg = format!(
            "[Uika] invoke_function: callback_id {} not found (registry size = {})",
            callback_id, vec_len,
        );
        let bytes = msg.as_bytes();
        unsafe {
            crate::ffi_dispatch::logging_log(1, bytes.as_ptr(), bytes.len() as u32);
        }
    }
}

/// Clear all registries and drop all instance data.
/// Called during shutdown before DLL unload (enables hot reload).
pub fn clear_all() {
    // 1. Drop all instance data, using the type's drop_fn.
    if let Some(instances) = INSTANCE_DATA.get() {
        let mut map = instances.write().unwrap();
        let types = type_registry().lock().unwrap();
        for (_, entry) in map.drain() {
            if let Some(info) = types.get(&entry.type_id) {
                unsafe {
                    (info.drop_fn)(entry.data);
                }
            }
        }
        drop(types);
    }
    // 2. Clear function registry.
    if let Some(funcs) = FUNC_REGISTRY.get() {
        funcs.write().unwrap().clear();
    }
    // 3. Clear type registry.
    if let Some(types) = TYPE_REGISTRY.get() {
        types.lock().unwrap().clear();
    }
}

/// Get the Rust instance data pointer for a UObject.
/// Returns null if no instance data is registered.
pub fn get_instance_data(obj: UObjectHandle) -> *mut u8 {
    let key = handle_to_key(obj);
    instance_data()
        .read()
        .unwrap()
        .get(&key)
        .map(|e| e.data)
        .unwrap_or(std::ptr::null_mut())
}
