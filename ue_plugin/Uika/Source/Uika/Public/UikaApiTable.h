#pragma once

// UikaApiTable.h — C++ half of the Rust ↔ C++ FFI contract.
// Every struct here must be layout-identical to the corresponding
// #[repr(C)] definition in uika-ffi/src/.

#include "CoreMinimal.h"

// ---------------------------------------------------------------------------
// Handle types (opaque, never dereferenced on the Rust side)
// ---------------------------------------------------------------------------

struct UikaUObjectHandle  { void* ptr; };
struct UikaUClassHandle   { void* ptr; };
struct UikaFPropertyHandle { void* ptr; };
struct UikaUFunctionHandle { void* ptr; };
struct UikaUStructHandle  { void* ptr; };
struct UikaFNameHandle    { uint64 value; };
struct UikaFWeakObjectHandle { int32 object_index; int32 object_serial_number; };

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

enum class EUikaErrorCode : uint32
{
    Ok              = 0,
    ObjectDestroyed = 1,
    InvalidCast     = 2,
    PropertyNotFound = 3,
    FunctionNotFound = 4,
    TypeMismatch    = 5,
    NullArgument    = 6,
    IndexOutOfRange = 7,
    InvalidOperation = 8,
    InternalError   = 9,
    BufferTooSmall  = 10,
};

// ---------------------------------------------------------------------------
// UikaCoreApi
// ---------------------------------------------------------------------------

struct FUikaCoreApi
{
    bool  (*is_valid)(UikaUObjectHandle obj);
    EUikaErrorCode (*get_name)(UikaUObjectHandle obj, uint8* buf, uint32 buf_len, uint32* out_len);
    UikaUClassHandle (*get_class)(UikaUObjectHandle obj);
    bool  (*is_a)(UikaUObjectHandle obj, UikaUClassHandle target_class);
    UikaUObjectHandle (*get_outer)(UikaUObjectHandle obj);

    // FName construction / conversion
    UikaFNameHandle (*make_fname)(const uint8* name_utf8, uint32 name_len);
    EUikaErrorCode  (*fname_to_string)(UikaFNameHandle handle, uint8* buf, uint32 buf_len, uint32* out_len);

    // Weak object pointers
    UikaFWeakObjectHandle (*make_weak)(UikaUObjectHandle obj);
    UikaUObjectHandle     (*resolve_weak)(UikaFWeakObjectHandle weak);
    bool                  (*is_weak_valid)(UikaFWeakObjectHandle weak);
};

// ---------------------------------------------------------------------------
// UikaLoggingApi
// ---------------------------------------------------------------------------

struct FUikaLoggingApi
{
    // level: 0=Display, 1=Warning, 2=Error.  msg is UTF-8 (not null-terminated).
    void (*log)(uint8 level, const uint8* msg, uint32 msg_len);
};

// ---------------------------------------------------------------------------
// UikaLifecycleApi
// ---------------------------------------------------------------------------

struct FUikaLifecycleApi
{
    void (*add_gc_root)(UikaUObjectHandle obj);
    void (*remove_gc_root)(UikaUObjectHandle obj);
    void (*register_pinned)(UikaUObjectHandle obj);
    void (*unregister_pinned)(UikaUObjectHandle obj);
};

// ---------------------------------------------------------------------------
// UikaPropertyApi
// ---------------------------------------------------------------------------

struct FUikaPropertyApi
{
    // Bool
    EUikaErrorCode (*get_bool)(UikaUObjectHandle obj, UikaFPropertyHandle prop, bool* out);
    EUikaErrorCode (*set_bool)(UikaUObjectHandle obj, UikaFPropertyHandle prop, bool val);
    // int32
    EUikaErrorCode (*get_i32)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int32* out);
    EUikaErrorCode (*set_i32)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int32 val);
    // int64
    EUikaErrorCode (*get_i64)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int64* out);
    EUikaErrorCode (*set_i64)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int64 val);
    // uint8
    EUikaErrorCode (*get_u8)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint8* out);
    EUikaErrorCode (*set_u8)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint8 val);
    // float
    EUikaErrorCode (*get_f32)(UikaUObjectHandle obj, UikaFPropertyHandle prop, float* out);
    EUikaErrorCode (*set_f32)(UikaUObjectHandle obj, UikaFPropertyHandle prop, float val);
    // double
    EUikaErrorCode (*get_f64)(UikaUObjectHandle obj, UikaFPropertyHandle prop, double* out);
    EUikaErrorCode (*set_f64)(UikaUObjectHandle obj, UikaFPropertyHandle prop, double val);
    // String (UTF-8)
    EUikaErrorCode (*get_string)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint8* buf, uint32 buf_len, uint32* out_len);
    EUikaErrorCode (*set_string)(UikaUObjectHandle obj, UikaFPropertyHandle prop, const uint8* buf, uint32 len);
    // FName
    EUikaErrorCode (*get_fname)(UikaUObjectHandle obj, UikaFPropertyHandle prop, UikaFNameHandle* out);
    EUikaErrorCode (*set_fname)(UikaUObjectHandle obj, UikaFPropertyHandle prop, UikaFNameHandle val);
    // Object reference
    EUikaErrorCode (*get_object)(UikaUObjectHandle obj, UikaFPropertyHandle prop, UikaUObjectHandle* out);
    EUikaErrorCode (*set_object)(UikaUObjectHandle obj, UikaFPropertyHandle prop, UikaUObjectHandle val);
    // Enum (as int64)
    EUikaErrorCode (*get_enum)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int64* out);
    EUikaErrorCode (*set_enum)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int64 val);
    // Struct (raw memory copy)
    EUikaErrorCode (*get_struct)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint8* out_buf, uint32 buf_size);
    EUikaErrorCode (*set_struct)(UikaUObjectHandle obj, UikaFPropertyHandle prop, const uint8* in_buf, uint32 buf_size);

    // Indexed access for fixed arrays (array_dim > 1).
    // Uses CopySingleValue internally — works for bool, numeric, enum, struct, object.
    // NOT safe for string/name/text types (requires constructed FString at dest).
    EUikaErrorCode (*get_property_at)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        uint32 index, uint8* out_buf, uint32 buf_size);
    EUikaErrorCode (*set_property_at)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        uint32 index, const uint8* in_buf, uint32 buf_size);
};

// ---------------------------------------------------------------------------
// UikaReflectionApi
// ---------------------------------------------------------------------------

struct FUikaReflectionApi
{
    UikaUClassHandle   (*find_class)(const uint8* name, uint32 name_len);
    UikaFPropertyHandle (*find_property)(UikaUClassHandle cls, const uint8* name, uint32 name_len);
    UikaUClassHandle   (*get_static_class)(const uint8* name, uint32 name_len);
    uint32             (*get_property_size)(UikaFPropertyHandle prop);

    // Struct reflection
    UikaUStructHandle  (*find_struct)(const uint8* name, uint32 name_len);
    UikaFPropertyHandle (*find_struct_property)(UikaUStructHandle ustruct, const uint8* name, uint32 name_len);

    // Reflection call support (for functions not in func_table)
    UikaUFunctionHandle (*find_function)(UikaUObjectHandle obj, const uint8* name, uint32 name_len);
    uint8*             (*alloc_params)(UikaUFunctionHandle func);
    void               (*free_params)(UikaUFunctionHandle func, uint8* params);
    EUikaErrorCode     (*call_function)(UikaUObjectHandle obj, UikaUFunctionHandle func, uint8* params);
    UikaFPropertyHandle (*get_function_param)(UikaUFunctionHandle func, const uint8* name, uint32 name_len);
    uint32             (*get_property_offset)(UikaFPropertyHandle prop);

    // Find a UFunction by class (no instance needed — for OnceLock caching)
    UikaUFunctionHandle (*find_function_by_class)(UikaUClassHandle cls, const uint8* name, uint32 name_len);

    // Get the element size of a property (FProperty::ElementSize).
    // For scalar properties, equals get_property_size().
    // For fixed arrays (array_dim > 1), equals total_size / array_dim.
    uint32 (*get_element_size)(UikaFPropertyHandle prop);

    // Get the structure size of a UScriptStruct.
    uint32 (*get_struct_size)(UikaUStructHandle ustruct);

    // Initialize struct memory using UScriptStruct default constructor.
    EUikaErrorCode (*initialize_struct)(UikaUStructHandle ustruct, uint8* data);

    // Destroy struct memory (calls C++ destructors for non-trivial members).
    EUikaErrorCode (*destroy_struct)(UikaUStructHandle ustruct, uint8* data);
};

// ---------------------------------------------------------------------------
// Placeholder sub-tables (filled in later phases)
// ---------------------------------------------------------------------------

struct FUikaMemoryApi       { uint8 _opaque; };
struct FUikaContainerApi
{
    // -- TArray --
    int32 (*array_len)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    EUikaErrorCode (*array_get)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        int32 index, uint8* out_buf, uint32 buf_size, uint32* out_written);
    EUikaErrorCode (*array_set)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        int32 index, const uint8* in_buf, uint32 buf_size);
    EUikaErrorCode (*array_add)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* in_buf, uint32 buf_size);
    EUikaErrorCode (*array_remove)(UikaUObjectHandle obj, UikaFPropertyHandle prop, int32 index);
    EUikaErrorCode (*array_clear)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    uint32 (*array_element_size)(UikaFPropertyHandle prop);

    // -- TMap --
    int32 (*map_len)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    EUikaErrorCode (*map_find)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* key_buf, uint32 key_size,
        uint8* out_val_buf, uint32 val_size, uint32* out_written);
    EUikaErrorCode (*map_add)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* key_buf, uint32 key_size, const uint8* val_buf, uint32 val_size);
    EUikaErrorCode (*map_remove)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* key_buf, uint32 key_size);
    EUikaErrorCode (*map_clear)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    EUikaErrorCode (*map_get_pair)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        int32 logical_index,
        uint8* out_key_buf, uint32 key_buf_size, uint32* out_key_written,
        uint8* out_val_buf, uint32 val_buf_size, uint32* out_val_written);

    // -- TSet --
    int32 (*set_len)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    bool  (*set_contains)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* elem_buf, uint32 elem_size);
    EUikaErrorCode (*set_add)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* elem_buf, uint32 elem_size);
    EUikaErrorCode (*set_remove)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* elem_buf, uint32 elem_size);
    EUikaErrorCode (*set_clear)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    EUikaErrorCode (*set_get_element)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        int32 logical_index, uint8* out_buf, uint32 buf_size, uint32* out_written);

    // -- Temp container allocation (for function params) --
    void* (*alloc_temp)(UikaFPropertyHandle prop);
    void  (*free_temp)(UikaFPropertyHandle prop, void* base);

    // -- Bulk copy/set (single FFI call for entire container) --
    // Format: [u32 written_1][data_1][u32 written_2][data_2]...
    EUikaErrorCode (*array_copy_all)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        uint8* out_buf, uint32 buf_size, uint32* out_total_written, int32* out_count);
    EUikaErrorCode (*array_set_all)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        const uint8* in_buf, uint32 buf_size, int32 count);
    EUikaErrorCode (*map_copy_all)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        uint8* out_buf, uint32 buf_size, uint32* out_total_written, int32* out_count);
    EUikaErrorCode (*set_copy_all)(UikaUObjectHandle obj, UikaFPropertyHandle prop,
        uint8* out_buf, uint32 buf_size, uint32* out_total_written, int32* out_count);
};
struct FUikaDelegateApi
{
    EUikaErrorCode (*bind_delegate)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint64 callback_id);
    EUikaErrorCode (*unbind_delegate)(UikaUObjectHandle obj, UikaFPropertyHandle prop);
    EUikaErrorCode (*add_multicast)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint64 callback_id);
    EUikaErrorCode (*remove_multicast)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint64 callback_id);
    EUikaErrorCode (*broadcast_multicast)(UikaUObjectHandle obj, UikaFPropertyHandle prop, uint8* params);
};
// ---------------------------------------------------------------------------
// Reify API types
// ---------------------------------------------------------------------------

enum class EUikaReifyPropType : uint32
{
    Bool = 0, Int8 = 1, Int16 = 2, Int32 = 3, Int64 = 4,
    UInt8 = 5, UInt16 = 6, UInt32 = 7, UInt64 = 8,
    Float = 9, Double = 10,
    String = 11, Name = 12, Text = 13,
    Object = 14, Class = 15, Struct = 16, Enum = 17,
};

struct FUikaReifyPropExtra
{
    UikaUClassHandle class_handle;      // Object/Class property class
    UikaUClassHandle meta_class_handle; // Class property metaclass
    UikaUStructHandle struct_handle;    // Struct property struct
    UikaUClassHandle enum_handle;       // Enum type (UEnum* cast)
    uint32 enum_underlying;             // Enum backing type
};

// ---------------------------------------------------------------------------
// FUikaReifyApi — runtime class creation, property/function registration
// ---------------------------------------------------------------------------

struct FUikaReifyApi
{
    UikaUClassHandle (*create_class)(
        const uint8* name, uint32 name_len,
        UikaUClassHandle parent,
        uint64 rust_type_id);

    UikaFPropertyHandle (*add_property)(
        UikaUClassHandle cls,
        const uint8* name, uint32 name_len,
        uint32 prop_type, uint64 prop_flags,
        const FUikaReifyPropExtra* extra);

    UikaUFunctionHandle (*add_function)(
        UikaUClassHandle cls,
        const uint8* name, uint32 name_len,
        uint64 callback_id, uint32 func_flags);

    EUikaErrorCode (*add_function_param)(
        UikaUFunctionHandle func,
        const uint8* name, uint32 name_len,
        uint32 prop_type, uint64 param_flags,
        const FUikaReifyPropExtra* extra);

    EUikaErrorCode (*finalize_class)(UikaUClassHandle cls);

    UikaUObjectHandle (*get_cdo)(UikaUClassHandle cls);

    EUikaErrorCode (*add_default_subobject)(
        UikaUClassHandle cls,
        const uint8* name, uint32 name_len,
        UikaUClassHandle component_class,
        uint32 flags,
        const uint8* attach_parent, uint32 attach_len);

    UikaUObjectHandle (*find_default_subobject)(
        UikaUObjectHandle owner,
        const uint8* name, uint32 name_len);
};
struct FUikaWorldApi
{
    UikaUObjectHandle (*spawn_actor)(UikaUObjectHandle world, UikaUClassHandle cls,
        const uint8* transform_buf, uint32 transform_size, UikaUObjectHandle owner);
    EUikaErrorCode (*get_all_actors_of_class)(UikaUObjectHandle world, UikaUClassHandle cls,
        uint8* out_buf, uint32 buf_byte_size, uint32* out_count);
    UikaUObjectHandle (*find_object)(UikaUClassHandle cls, const uint8* path_utf8, uint32 path_len);
    UikaUObjectHandle (*load_object)(UikaUClassHandle cls, const uint8* path_utf8, uint32 path_len);
    UikaUObjectHandle (*get_world)(UikaUObjectHandle actor);

    // Create a new UObject. outer can be null (falls back to transient package).
    UikaUObjectHandle (*new_object)(UikaUObjectHandle outer, UikaUClassHandle cls);

    // Spawn an actor with deferred construction (BeginPlay not yet called).
    // collision_method maps to ESpawnActorCollisionHandlingMethod.
    UikaUObjectHandle (*spawn_actor_deferred)(UikaUObjectHandle world, UikaUClassHandle cls,
        const uint8* transform_buf, uint32 transform_size,
        UikaUObjectHandle owner, UikaUObjectHandle instigator, uint8 collision_method);

    // Finish spawning a deferred actor (triggers BeginPlay).
    EUikaErrorCode (*finish_spawning)(UikaUObjectHandle actor,
        const uint8* transform_buf, uint32 transform_size);
};

// ---------------------------------------------------------------------------
// Main API table
// ---------------------------------------------------------------------------

struct FUikaApiTable
{
    uint32 version;

    // Fixed sub-tables
    const FUikaCoreApi*         core;
    const FUikaPropertyApi*     property;
    const FUikaReflectionApi*   reflection;
    const FUikaMemoryApi*       memory;
    const FUikaContainerApi*    container;
    const FUikaDelegateApi*     delegate;
    const FUikaLifecycleApi*    lifecycle;
    const FUikaReifyApi*        reify;
    const FUikaWorldApi*        world;
    const FUikaLoggingApi*      logging;

    // Generated function-pointer array
    const void* const*          func_table;
    uint32                      func_count;
};

// ---------------------------------------------------------------------------
// Rust callback table (returned by uika_init)
// ---------------------------------------------------------------------------

struct FUikaRustCallbacks
{
    void (*drop_rust_instance)(UikaUObjectHandle handle, uint64 type_id, uint8* rust_data);
    void (*invoke_rust_function)(uint64 callback_id, UikaUObjectHandle obj, uint8* params);
    void (*invoke_delegate_callback)(uint64 callback_id, uint8* params);
    void (*on_shutdown)();
    void (*construct_rust_instance)(UikaUObjectHandle obj, uint64 type_id, bool is_cdo);
    void (*notify_pinned_destroyed)(UikaUObjectHandle handle);
};

// ---------------------------------------------------------------------------
// DLL function signatures
// ---------------------------------------------------------------------------

using FUikaInitFn     = const FUikaRustCallbacks* (*)(const FUikaApiTable* api_table);
using FUikaShutdownFn = void (*)();
