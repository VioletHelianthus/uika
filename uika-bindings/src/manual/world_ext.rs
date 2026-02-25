// Type-safe gameplay wrappers on top of uika_runtime::world raw functions.

use uika_runtime::{OwnedStruct, UObjectRef, UeClass, UikaResult};

use crate::core_ue::FTransform;
use crate::engine::{Actor, ActorExt, World};

/// Emit a warning if the spawned actor has no root component.
///
/// Skips base engine classes (AActor, APawn) where lacking a RootComponent
/// is by design. Only warns for user subclasses that likely should have one.
fn warn_no_root_component(handle: uika_ffi::UObjectHandle) {
    use crate::engine::Pawn;

    let actor_ref: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(handle) };
    if let Ok(actual_class) = actor_ref.get_class() {
        // Base engine classes intentionally have no RootComponent
        if actual_class == Actor::static_class()
            || actual_class == Pawn::static_class()
        {
            return;
        }
    }
    if let Ok(checked) = actor_ref.checked() {
        let root = checked.k2_get_root_component();
        if !root.is_valid() {
            uika_runtime::ulog!(uika_runtime::LOG_WARNING,
                "[Uika] spawn_actor: spawned actor has no RootComponent — \
                 spawn transform will be ignored and set_actor_location will fail. \
                 Consider inheriting from a class with a default RootComponent."
            );
        }
    }
}

/// Collision handling method for spawn operations.
/// Maps to UE's `ESpawnActorCollisionHandlingMethod`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default)]
pub enum SpawnCollisionMethod {
    #[default]
    Undefined = 0,
    AlwaysSpawn = 1,
    AdjustIfPossibleButAlwaysSpawn = 2,
    AdjustIfPossibleButDontSpawnIfColliding = 3,
    DontSpawnIfColliding = 4,
}

/// Extension trait for spawning and querying actors in a UWorld.
pub trait WorldSpawnExt {
    fn spawn_actor<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<UObjectRef<T>>;

    fn spawn_actor_with_owner<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
        owner: &UObjectRef<Actor>,
    ) -> UikaResult<UObjectRef<T>>;

    fn spawn_actor_deferred<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<UObjectRef<T>>;

    fn spawn_actor_deferred_full<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
        owner: uika_ffi::UObjectHandle,
        instigator: uika_ffi::UObjectHandle,
        collision_method: SpawnCollisionMethod,
    ) -> UikaResult<UObjectRef<T>>;

    fn finish_spawning(
        &self,
        actor: &UObjectRef<Actor>,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<()>;

    fn get_all_actors_of_class<T: UeClass>(&self) -> UikaResult<Vec<UObjectRef<T>>>;
}

impl WorldSpawnExt for UObjectRef<World> {
    fn spawn_actor<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<UObjectRef<T>> {
        let world = self.checked()?.raw();
        let class = T::static_class();
        let null_owner = uika_ffi::UObjectHandle(std::ptr::null_mut());
        let handle = uika_runtime::world::spawn_actor_raw(
            world,
            class,
            transform.as_bytes(),
            null_owner,
        )?;
        warn_no_root_component(handle);
        Ok(unsafe { UObjectRef::from_raw(handle) })
    }

    fn spawn_actor_with_owner<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
        owner: &UObjectRef<Actor>,
    ) -> UikaResult<UObjectRef<T>> {
        let world = self.checked()?.raw();
        let class = T::static_class();
        let owner_handle = owner.checked()?.raw();
        let handle = uika_runtime::world::spawn_actor_raw(
            world,
            class,
            transform.as_bytes(),
            owner_handle,
        )?;
        warn_no_root_component(handle);
        Ok(unsafe { UObjectRef::from_raw(handle) })
    }

    fn spawn_actor_deferred<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<UObjectRef<T>> {
        let world = self.checked()?.raw();
        let class = T::static_class();
        let null = uika_ffi::UObjectHandle(std::ptr::null_mut());
        let handle = uika_runtime::world::spawn_actor_deferred_raw(
            world, class, transform.as_bytes(), null, null, 0,
        )?;
        warn_no_root_component(handle);
        Ok(unsafe { UObjectRef::from_raw(handle) })
    }

    fn spawn_actor_deferred_full<T: UeClass>(
        &self,
        transform: &OwnedStruct<FTransform>,
        owner: uika_ffi::UObjectHandle,
        instigator: uika_ffi::UObjectHandle,
        collision_method: SpawnCollisionMethod,
    ) -> UikaResult<UObjectRef<T>> {
        let world = self.checked()?.raw();
        let class = T::static_class();
        let handle = uika_runtime::world::spawn_actor_deferred_raw(
            world, class, transform.as_bytes(), owner, instigator, collision_method as u8,
        )?;
        warn_no_root_component(handle);
        Ok(unsafe { UObjectRef::from_raw(handle) })
    }

    fn finish_spawning(
        &self,
        actor: &UObjectRef<Actor>,
        transform: &OwnedStruct<FTransform>,
    ) -> UikaResult<()> {
        let actor_handle = actor.checked()?.raw();
        uika_runtime::world::finish_spawning_raw(actor_handle, transform.as_bytes())
    }

    fn get_all_actors_of_class<T: UeClass>(&self) -> UikaResult<Vec<UObjectRef<T>>> {
        let world = self.checked()?.raw();
        let class = T::static_class();
        let handles = uika_runtime::world::get_all_actors_of_class_raw(world, class)?;
        Ok(handles
            .into_iter()
            .map(|h| unsafe { UObjectRef::from_raw(h) })
            .collect())
    }
}

/// Find an already-loaded object by class and path.
pub fn find_object<T: UeClass>(path: &str) -> UikaResult<UObjectRef<T>> {
    let class = T::static_class();
    let handle = uika_runtime::world::find_object_raw(class, path)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Load an object by class and path (triggers load if not already loaded).
pub fn load_object<T: UeClass>(path: &str) -> UikaResult<UObjectRef<T>> {
    let class = T::static_class();
    let handle = uika_runtime::world::load_object_raw(class, path)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Create a new UObject of the given class, parented to `outer`.
pub fn new_object<T: UeClass>(outer: &UObjectRef<impl UeClass>) -> UikaResult<UObjectRef<T>> {
    let outer_handle = outer.checked()?.raw();
    let class = T::static_class();
    let handle = uika_runtime::world::new_object_raw(outer_handle, class)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Create a new UObject of the given class in the transient package.
pub fn new_object_transient<T: UeClass>() -> UikaResult<UObjectRef<T>> {
    let class = T::static_class();
    let null_outer = uika_ffi::UObjectHandle(std::ptr::null_mut());
    let handle = uika_runtime::world::new_object_raw(null_outer, class)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Spawn an actor using a runtime `UClassHandle` (e.g. from TSubclassOf).
///
/// Returns an untyped `UObjectHandle` — caller casts via `UObjectRef::from_raw`.
pub fn spawn_actor_dynamic(
    world: &UObjectRef<World>,
    class: uika_ffi::UClassHandle,
    transform: &OwnedStruct<FTransform>,
) -> UikaResult<uika_ffi::UObjectHandle> {
    let world_handle = world.checked()?.raw();
    let null_owner = uika_ffi::UObjectHandle(std::ptr::null_mut());
    uika_runtime::world::spawn_actor_raw(world_handle, class, transform.as_bytes(), null_owner)
}

/// Create a new UObject using a runtime `UClassHandle` (e.g. from TSubclassOf).
///
/// Returns an untyped `UObjectHandle` — caller casts via `UObjectRef::from_raw`.
pub fn new_object_dynamic(
    outer: uika_ffi::UObjectHandle,
    class: uika_ffi::UClassHandle,
) -> UikaResult<uika_ffi::UObjectHandle> {
    uika_runtime::world::new_object_raw(outer, class)
}
