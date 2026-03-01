// World-level gameplay template function wrappers (raw handle versions).
// Type-safe wrappers live in uika-bindings/src/manual/world_ext.rs.

use uika_ffi::{UClassHandle, UObjectHandle};

use crate::error::{check_ffi, UikaError, UikaResult};
use crate::ffi_dispatch;

/// Spawn an actor in the world.
///
/// - `world`: the UWorld handle
/// - `class`: the UClass of the actor to spawn
/// - `transform_buf`: raw bytes of an FTransform struct
/// - `owner`: optional owning actor (null handle for none)
pub fn spawn_actor_raw(
    world: UObjectHandle,
    class: UClassHandle,
    transform_buf: &[u8],
    owner: UObjectHandle,
) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::world_spawn_actor(
            world,
            class,
            transform_buf.as_ptr(),
            transform_buf.len() as u32,
            owner,
        )
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation("spawn_actor returned null".into()))
    } else {
        Ok(result)
    }
}

/// Find a UObject by class and path (already loaded objects only).
pub fn find_object_raw(class: UClassHandle, path: &str) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::world_find_object(class, path.as_ptr(), path.len() as u32)
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation(format!("find_object: not found: {path}")))
    } else {
        Ok(result)
    }
}

/// Load a UObject by class and path (triggers load if not already loaded).
pub fn load_object_raw(class: UClassHandle, path: &str) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::world_load_object(class, path.as_ptr(), path.len() as u32)
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation(format!("load_object: failed to load: {path}")))
    } else {
        Ok(result)
    }
}

/// Create a new UObject of the given class.
///
/// - `outer`: the outer object (null handle = transient package)
/// - `class`: the UClass to instantiate
pub fn new_object_raw(outer: UObjectHandle, class: UClassHandle) -> UikaResult<UObjectHandle> {
    let result = unsafe { ffi_dispatch::world_new_object(outer, class) };
    if result.is_null() {
        Err(UikaError::InvalidOperation("new_object returned null".into()))
    } else {
        Ok(result)
    }
}

/// Spawn an actor with deferred construction (BeginPlay not yet called).
///
/// The returned actor can be configured before calling `finish_spawning_raw`.
/// `collision_method` maps to `ESpawnActorCollisionHandlingMethod` (0..4).
pub fn spawn_actor_deferred_raw(
    world: UObjectHandle,
    class: UClassHandle,
    transform_buf: &[u8],
    owner: UObjectHandle,
    instigator: UObjectHandle,
    collision_method: u8,
) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::world_spawn_actor_deferred(
            world,
            class,
            transform_buf.as_ptr(),
            transform_buf.len() as u32,
            owner,
            instigator,
            collision_method,
        )
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation("spawn_actor_deferred returned null".into()))
    } else {
        Ok(result)
    }
}

/// Finish spawning a deferred actor (triggers BeginPlay).
pub fn finish_spawning_raw(
    actor: UObjectHandle,
    transform_buf: &[u8],
) -> UikaResult<()> {
    check_ffi(unsafe {
        ffi_dispatch::world_finish_spawning(
            actor,
            transform_buf.as_ptr(),
            transform_buf.len() as u32,
        )
    })
}

/// Get the UWorld from an actor handle.
pub fn get_world_raw(actor: UObjectHandle) -> UikaResult<UObjectHandle> {
    let result = unsafe { ffi_dispatch::world_get_world(actor) };
    if result.is_null() {
        Err(UikaError::InvalidOperation("get_world returned null".into()))
    } else {
        Ok(result)
    }
}

/// Get all actors of a given class in the world.
pub fn get_all_actors_of_class_raw(
    world: UObjectHandle,
    class: UClassHandle,
) -> UikaResult<Vec<UObjectHandle>> {
    let handle_size = core::mem::size_of::<UObjectHandle>() as u32;

    // First call with zero capacity to get the count.
    let mut count: u32 = 0;
    check_ffi(unsafe {
        ffi_dispatch::world_get_all_actors_of_class(
            world,
            class,
            std::ptr::null_mut(),
            0,
            &mut count,
        )
    })?;

    if count == 0 {
        return Ok(Vec::new());
    }

    // Second call: allocate a byte buffer and pass byte size.
    let byte_size = count * handle_size;
    let mut buf = vec![0u8; byte_size as usize];
    let mut actual_count: u32 = 0;
    check_ffi(unsafe {
        ffi_dispatch::world_get_all_actors_of_class(
            world,
            class,
            buf.as_mut_ptr(),
            byte_size,
            &mut actual_count,
        )
    })?;

    // Reinterpret byte buffer as UObjectHandle array.
    let handles = buf
        .chunks_exact(handle_size as usize)
        .take(actual_count as usize)
        .map(|chunk| {
            let bytes: [u8; 8] = chunk.try_into().expect("handle is 8 bytes");
            #[cfg(not(target_arch = "wasm32"))]
            {
                let ptr = usize::from_ne_bytes(bytes) as *mut std::ffi::c_void;
                UObjectHandle(ptr)
            }
            #[cfg(target_arch = "wasm32")]
            {
                UObjectHandle(u64::from_ne_bytes(bytes))
            }
        })
        .collect();

    Ok(handles)
}
