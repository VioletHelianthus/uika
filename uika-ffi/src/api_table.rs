use std::ffi::c_void;

use crate::error::UikaErrorCode;
use crate::handles::*;
use crate::reify_types::UikaReifyPropExtra;

// Re-export FWeakObjectHandle for use by api_table consumers.
pub use crate::handles::FWeakObjectHandle;

// ---------------------------------------------------------------------------
// Main API table
// ---------------------------------------------------------------------------

/// The top-level table passed from C++ to Rust at init time.
/// Two tiers: fixed sub-tables (hand-written infrastructure) and a generated
/// function-pointer flat array (one entry per codegen-exported UE function).
#[repr(C)]
pub struct UikaApiTable {
    pub version: u32,

    // ---- Fixed sub-tables (hand-written, infrastructure) ----
    pub core: *const UikaCoreApi,
    pub property: *const UikaPropertyApi,
    pub reflection: *const UikaReflectionApi,
    pub memory: *const UikaMemoryApi,
    pub container: *const UikaContainerApi,
    pub delegate: *const UikaDelegateApi,
    pub lifecycle: *const UikaLifecycleApi,
    pub reify: *const UikaReifyApi,
    pub world: *const UikaWorldApi,
    pub logging: *const UikaLoggingApi,

    // ---- Generated function-pointer array (codegen) ----
    /// Flat array indexed by codegen-assigned FuncId. Each pointer targets a
    /// generated C++ wrapper that directly calls the UE C++ API.
    pub func_table: *const *const c_void,
    pub func_count: u32,
}

unsafe impl Send for UikaApiTable {}
unsafe impl Sync for UikaApiTable {}

// ---------------------------------------------------------------------------
// UikaCoreApi
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct UikaCoreApi {
    /// Check whether a UObject is still alive (queries GUObjectArray).
    pub is_valid: unsafe extern "C" fn(obj: UObjectHandle) -> bool,

    /// Write the UObject's FName into a caller-supplied buffer (UTF-8).
    pub get_name: unsafe extern "C" fn(
        obj: UObjectHandle,
        buf: *mut u8,
        buf_len: u32,
        out_len: *mut u32,
    ) -> UikaErrorCode,

    /// Get the UObject's UClass.
    pub get_class: unsafe extern "C" fn(obj: UObjectHandle) -> UClassHandle,

    /// IsA check.
    pub is_a: unsafe extern "C" fn(obj: UObjectHandle, target_class: UClassHandle) -> bool,

    /// Get the UObject's Outer.
    pub get_outer: unsafe extern "C" fn(obj: UObjectHandle) -> UObjectHandle,

    // -- FName construction / conversion --

    /// Create an FName from a UTF-8 string.
    pub make_fname: unsafe extern "C" fn(name_utf8: *const u8, name_len: u32) -> FNameHandle,

    /// Convert an FName to a UTF-8 string. Writes into caller-supplied buffer.
    pub fname_to_string: unsafe extern "C" fn(
        handle: FNameHandle,
        buf: *mut u8,
        buf_len: u32,
        out_len: *mut u32,
    ) -> UikaErrorCode,

    // -- Weak object pointers --

    /// Create a weak pointer from a strong UObject reference.
    pub make_weak: unsafe extern "C" fn(obj: UObjectHandle) -> FWeakObjectHandle,

    /// Resolve a weak pointer to a strong reference. Returns null handle if expired.
    pub resolve_weak: unsafe extern "C" fn(weak: FWeakObjectHandle) -> UObjectHandle,

    /// Check if a weak pointer is still valid (without resolving).
    pub is_weak_valid: unsafe extern "C" fn(weak: FWeakObjectHandle) -> bool,
}

// ---------------------------------------------------------------------------
// UikaLoggingApi
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct UikaLoggingApi {
    /// Bridge to UE_LOG. `level`: 0=Display, 1=Warning, 2=Error.
    /// `msg` is a UTF-8 byte slice (not null-terminated).
    pub log: unsafe extern "C" fn(level: u8, msg: *const u8, msg_len: u32),
}

// ---------------------------------------------------------------------------
// UikaLifecycleApi
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct UikaLifecycleApi {
    /// Add a GC root (prevents UE garbage collection).
    pub add_gc_root: unsafe extern "C" fn(obj: UObjectHandle),
    /// Remove a GC root.
    pub remove_gc_root: unsafe extern "C" fn(obj: UObjectHandle),
    /// Register a Pinned object for destroy notification (alive flag).
    pub register_pinned: unsafe extern "C" fn(obj: UObjectHandle),
    /// Unregister a Pinned object from destroy notification.
    pub unregister_pinned: unsafe extern "C" fn(obj: UObjectHandle),
}

// ---------------------------------------------------------------------------
// UikaPropertyApi
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct UikaPropertyApi {
    // -- Boolean --
    pub get_bool: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut bool) -> UikaErrorCode,
    pub set_bool: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: bool) -> UikaErrorCode,

    // -- Integers --
    pub get_i32: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut i32) -> UikaErrorCode,
    pub set_i32: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: i32) -> UikaErrorCode,
    pub get_i64: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut i64) -> UikaErrorCode,
    pub set_i64: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: i64) -> UikaErrorCode,
    pub get_u8: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut u8) -> UikaErrorCode,
    pub set_u8: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: u8) -> UikaErrorCode,

    // -- Floating point --
    pub get_f32: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut f32) -> UikaErrorCode,
    pub set_f32: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: f32) -> UikaErrorCode,
    pub get_f64: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut f64) -> UikaErrorCode,
    pub set_f64: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: f64) -> UikaErrorCode,

    // -- String (UTF-8 buffer) --
    pub get_string: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, buf: *mut u8, buf_len: u32, out_len: *mut u32) -> UikaErrorCode,
    pub set_string: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, buf: *const u8, len: u32) -> UikaErrorCode,

    // -- FName --
    pub get_fname: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut FNameHandle) -> UikaErrorCode,
    pub set_fname: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: FNameHandle) -> UikaErrorCode,

    // -- Object reference --
    pub get_object: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut UObjectHandle) -> UikaErrorCode,
    pub set_object: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: UObjectHandle) -> UikaErrorCode,

    // -- Enum (as underlying integer) --
    pub get_enum: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out: *mut i64) -> UikaErrorCode,
    pub set_enum: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, val: i64) -> UikaErrorCode,

    // -- Struct (memory copy) --
    pub get_struct: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, out_buf: *mut u8, buf_size: u32) -> UikaErrorCode,
    pub set_struct: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle, in_buf: *const u8, buf_size: u32) -> UikaErrorCode,

    // -- Indexed access (fixed arrays) --
    pub get_property_at: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        index: u32, out_buf: *mut u8, buf_size: u32,
    ) -> UikaErrorCode,
    pub set_property_at: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        index: u32, in_buf: *const u8, buf_size: u32,
    ) -> UikaErrorCode,
}

// ---------------------------------------------------------------------------
// UikaReflectionApi
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct UikaReflectionApi {
    /// Find a UClass by name.
    pub find_class: unsafe extern "C" fn(name: *const u8, name_len: u32) -> UClassHandle,

    /// Find an FProperty on a UClass by name.
    pub find_property: unsafe extern "C" fn(
        class: UClassHandle,
        name: *const u8,
        name_len: u32,
    ) -> FPropertyHandle,

    /// Get the StaticClass handle for a class by name.
    pub get_static_class: unsafe extern "C" fn(name: *const u8, name_len: u32) -> UClassHandle,

    /// Get the size of a property.
    pub get_property_size: unsafe extern "C" fn(prop: FPropertyHandle) -> u32,

    /// Find a UScriptStruct by name.
    pub find_struct: unsafe extern "C" fn(name: *const u8, name_len: u32) -> UStructHandle,

    /// Find an FProperty on a UScriptStruct by name.
    pub find_struct_property: unsafe extern "C" fn(
        ustruct: UStructHandle,
        name: *const u8,
        name_len: u32,
    ) -> FPropertyHandle,

    // ---- Reflection call support (for functions not covered by codegen) ----

    /// Find a UFunction by name on an object's class.
    pub find_function: unsafe extern "C" fn(
        obj: UObjectHandle,
        name: *const u8,
        name_len: u32,
    ) -> UFunctionHandle,

    /// Allocate the parameter buffer for a UFunction (size = ParmsSize, zero-initialized).
    pub alloc_params: unsafe extern "C" fn(func: UFunctionHandle) -> *mut u8,

    /// Free the parameter buffer.
    pub free_params: unsafe extern "C" fn(func: UFunctionHandle, params: *mut u8),

    /// Call a UFunction via ProcessEvent.
    pub call_function: unsafe extern "C" fn(
        obj: UObjectHandle,
        func: UFunctionHandle,
        params: *mut u8,
    ) -> UikaErrorCode,

    /// Get an FProperty for a UFunction parameter (to locate offsets in the buffer).
    pub get_function_param: unsafe extern "C" fn(
        func: UFunctionHandle,
        name: *const u8,
        name_len: u32,
    ) -> FPropertyHandle,

    /// Get the offset of an FProperty within its container.
    pub get_property_offset: unsafe extern "C" fn(prop: FPropertyHandle) -> u32,

    /// Find a UFunction by class handle (no instance needed — for OnceLock caching).
    pub find_function_by_class: unsafe extern "C" fn(
        cls: UClassHandle,
        name: *const u8,
        name_len: u32,
    ) -> UFunctionHandle,

    /// Get the element size of a property (FProperty::ElementSize).
    pub get_element_size: unsafe extern "C" fn(prop: FPropertyHandle) -> u32,

    /// Get the structure size of a UScriptStruct (for OwnedStruct allocation).
    pub get_struct_size: unsafe extern "C" fn(ustruct: UStructHandle) -> u32,

    /// Initialize struct memory using the UScriptStruct's default constructor.
    pub initialize_struct: unsafe extern "C" fn(ustruct: UStructHandle, data: *mut u8) -> UikaErrorCode,

    /// Destroy struct memory (calls C++ destructors for non-trivial members).
    pub destroy_struct: unsafe extern "C" fn(ustruct: UStructHandle, data: *mut u8) -> UikaErrorCode,
}

// ---------------------------------------------------------------------------
// Placeholder sub-tables (filled in later phases)
// ---------------------------------------------------------------------------

/// Phase 7: TArray / TMap / TSet operations.
#[repr(C)]
pub struct UikaMemoryApi {
    _opaque: u8,
}

/// Phase 7: Container operations (TArray / TMap / TSet).
#[repr(C)]
pub struct UikaContainerApi {
    // -- TArray --
    pub array_len: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle) -> i32,
    pub array_get: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        index: i32, out_buf: *mut u8, buf_size: u32, out_written: *mut u32,
    ) -> UikaErrorCode,
    pub array_set: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        index: i32, in_buf: *const u8, buf_size: u32,
    ) -> UikaErrorCode,
    pub array_add: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        in_buf: *const u8, buf_size: u32,
    ) -> UikaErrorCode,
    pub array_remove: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle, index: i32,
    ) -> UikaErrorCode,
    pub array_clear: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
    ) -> UikaErrorCode,
    pub array_element_size: unsafe extern "C" fn(prop: FPropertyHandle) -> u32,

    // -- TMap --
    pub map_len: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle) -> i32,
    pub map_find: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        key_buf: *const u8, key_size: u32,
        out_val_buf: *mut u8, val_size: u32, out_written: *mut u32,
    ) -> UikaErrorCode,
    pub map_add: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        key_buf: *const u8, key_size: u32, val_buf: *const u8, val_size: u32,
    ) -> UikaErrorCode,
    pub map_remove: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        key_buf: *const u8, key_size: u32,
    ) -> UikaErrorCode,
    pub map_clear: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
    ) -> UikaErrorCode,
    pub map_get_pair: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        logical_index: i32,
        out_key_buf: *mut u8, key_buf_size: u32, out_key_written: *mut u32,
        out_val_buf: *mut u8, val_buf_size: u32, out_val_written: *mut u32,
    ) -> UikaErrorCode,

    // -- TSet --
    pub set_len: unsafe extern "C" fn(obj: UObjectHandle, prop: FPropertyHandle) -> i32,
    pub set_contains: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        elem_buf: *const u8, elem_size: u32,
    ) -> bool,
    pub set_add: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        elem_buf: *const u8, elem_size: u32,
    ) -> UikaErrorCode,
    pub set_remove: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        elem_buf: *const u8, elem_size: u32,
    ) -> UikaErrorCode,
    pub set_clear: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
    ) -> UikaErrorCode,
    pub set_get_element: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        logical_index: i32, out_buf: *mut u8, buf_size: u32, out_written: *mut u32,
    ) -> UikaErrorCode,

    /// Allocate a temp container (for use as function parameter).
    /// Returns a base pointer such that ContainerPtrToValuePtr(base) points to
    /// an initialized empty container. Free with `free_temp`.
    pub alloc_temp: unsafe extern "C" fn(prop: FPropertyHandle) -> *mut u8,

    /// Free a temp container allocated by `alloc_temp`.
    pub free_temp: unsafe extern "C" fn(prop: FPropertyHandle, base: *mut u8),

    // -- Bulk copy/set (single FFI call for entire container) --
    // Buffer format: [u32 written_1][data_1][u32 written_2][data_2]...
    // For maps: [u32 key_written][key][u32 val_written][val] per pair.

    /// Copy all array elements into a flat buffer. Returns BufferTooSmall if
    /// the buffer is too small (out_total_written is set to the required size).
    pub array_copy_all: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        out_buf: *mut u8, buf_size: u32,
        out_total_written: *mut u32, out_count: *mut i32,
    ) -> UikaErrorCode,

    /// Replace all array elements from a flat buffer.
    pub array_set_all: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        in_buf: *const u8, buf_size: u32, count: i32,
    ) -> UikaErrorCode,

    /// Copy all map key-value pairs into a flat buffer.
    pub map_copy_all: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        out_buf: *mut u8, buf_size: u32,
        out_total_written: *mut u32, out_count: *mut i32,
    ) -> UikaErrorCode,

    /// Copy all set elements into a flat buffer.
    pub set_copy_all: unsafe extern "C" fn(
        obj: UObjectHandle, prop: FPropertyHandle,
        out_buf: *mut u8, buf_size: u32,
        out_total_written: *mut u32, out_count: *mut i32,
    ) -> UikaErrorCode,
}

/// Phase 8: Delegate binding / unbinding / broadcast.
#[repr(C)]
pub struct UikaDelegateApi {
    pub bind_delegate: unsafe extern "C" fn(
        obj: UObjectHandle,
        prop: FPropertyHandle,
        callback_id: u64,
    ) -> UikaErrorCode,
    pub unbind_delegate: unsafe extern "C" fn(
        obj: UObjectHandle,
        prop: FPropertyHandle,
    ) -> UikaErrorCode,
    pub add_multicast: unsafe extern "C" fn(
        obj: UObjectHandle,
        prop: FPropertyHandle,
        callback_id: u64,
    ) -> UikaErrorCode,
    pub remove_multicast: unsafe extern "C" fn(
        obj: UObjectHandle,
        prop: FPropertyHandle,
        callback_id: u64,
    ) -> UikaErrorCode,
    pub broadcast_multicast: unsafe extern "C" fn(
        obj: UObjectHandle,
        prop: FPropertyHandle,
        params: *mut u8,
    ) -> UikaErrorCode,
}

/// Phase 9: Reify — runtime class creation, property/function registration.
#[repr(C)]
pub struct UikaReifyApi {
    pub create_class: unsafe extern "C" fn(
        name: *const u8,
        name_len: u32,
        parent: UClassHandle,
        rust_type_id: u64,
    ) -> UClassHandle,

    pub add_property: unsafe extern "C" fn(
        cls: UClassHandle,
        name: *const u8,
        name_len: u32,
        prop_type: u32,
        prop_flags: u64,
        extra: *const UikaReifyPropExtra,
    ) -> FPropertyHandle,

    pub add_function: unsafe extern "C" fn(
        cls: UClassHandle,
        name: *const u8,
        name_len: u32,
        callback_id: u64,
        func_flags: u32,
    ) -> UFunctionHandle,

    pub add_function_param: unsafe extern "C" fn(
        func: UFunctionHandle,
        name: *const u8,
        name_len: u32,
        prop_type: u32,
        param_flags: u64,
        extra: *const UikaReifyPropExtra,
    ) -> UikaErrorCode,

    pub finalize_class: unsafe extern "C" fn(cls: UClassHandle) -> UikaErrorCode,

    pub get_cdo: unsafe extern "C" fn(cls: UClassHandle) -> UObjectHandle,

    /// Register a default subobject to be created during class construction.
    /// `flags`: bitfield — UIKA_COMP_ROOT=1, UIKA_COMP_TRANSIENT=2.
    /// `attach_parent`/`attach_len`: name of parent subobject (0-len = none).
    pub add_default_subobject: unsafe extern "C" fn(
        cls: UClassHandle,
        name: *const u8, name_len: u32,
        component_class: UClassHandle,
        flags: u32,
        attach_parent: *const u8, attach_len: u32,
    ) -> UikaErrorCode,

    /// Find a default subobject by name on an existing instance.
    pub find_default_subobject: unsafe extern "C" fn(
        owner: UObjectHandle,
        name: *const u8, name_len: u32,
    ) -> UObjectHandle,
}

pub const UIKA_COMP_ROOT: u32 = 1;
pub const UIKA_COMP_TRANSIENT: u32 = 2;

/// World-level queries (spawn, find actors, etc.).
#[repr(C)]
pub struct UikaWorldApi {
    /// Spawn an actor in the world. `transform_buf` must point to an FTransform-sized buffer.
    /// `owner` can be a null handle for no owner.
    pub spawn_actor: unsafe extern "C" fn(
        world: UObjectHandle,
        class: UClassHandle,
        transform_buf: *const u8,
        transform_size: u32,
        owner: UObjectHandle,
    ) -> UObjectHandle,

    /// Get all actors of a given class in the world. Returns actor count.
    /// Writes handles into `out_buf` (byte buffer, up to `buf_byte_size` bytes).
    /// Each handle is `size_of::<UObjectHandle>()` bytes.
    pub get_all_actors_of_class: unsafe extern "C" fn(
        world: UObjectHandle,
        class: UClassHandle,
        out_buf: *mut u8,
        buf_byte_size: u32,
        out_count: *mut u32,
    ) -> UikaErrorCode,

    /// Find an object by class and path. Returns null handle if not found.
    pub find_object: unsafe extern "C" fn(
        class: UClassHandle,
        path_utf8: *const u8,
        path_len: u32,
    ) -> UObjectHandle,

    /// Load an object by class and path (triggers load if needed). Returns null handle on failure.
    pub load_object: unsafe extern "C" fn(
        class: UClassHandle,
        path_utf8: *const u8,
        path_len: u32,
    ) -> UObjectHandle,

    /// Get the UWorld from an actor. Returns null handle if the actor is invalid.
    pub get_world: unsafe extern "C" fn(actor: UObjectHandle) -> UObjectHandle,

    /// Create a new UObject. `outer` can be null (falls back to transient package).
    pub new_object: unsafe extern "C" fn(outer: UObjectHandle, class: UClassHandle) -> UObjectHandle,

    /// Spawn an actor with deferred construction (BeginPlay not yet called).
    /// `collision_method` maps to ESpawnActorCollisionHandlingMethod (0=Undefined..4=DontSpawnIfColliding).
    /// `owner` and `instigator` can be null handles.
    pub spawn_actor_deferred: unsafe extern "C" fn(
        world: UObjectHandle,
        class: UClassHandle,
        transform_buf: *const u8,
        transform_size: u32,
        owner: UObjectHandle,
        instigator: UObjectHandle,
        collision_method: u8,
    ) -> UObjectHandle,

    /// Finish spawning a deferred actor (triggers BeginPlay).
    pub finish_spawning: unsafe extern "C" fn(
        actor: UObjectHandle,
        transform_buf: *const u8,
        transform_size: u32,
    ) -> UikaErrorCode,
}
