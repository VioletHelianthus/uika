// Prelude: one-import access to the most commonly used Uika types.
//
// Usage: `use uika::prelude::*;`

// Core runtime types
pub use uika_runtime::{
    UObjectRef, Pinned, UikaResult, UikaError, UeClass, UeStruct, UeEnum,
    OwnedStruct, UStructRef, UeArray, UeMap, UeSet,
    DynamicCall, DynamicCallResult, DelegateBinding,
    FName, TWeakObjectPtr,
    LOG_DISPLAY, LOG_WARNING, LOG_ERROR,
};

// UE math types (uika-runtime)
pub use uika_runtime::{
    Rotator, Transform, LinearColor, Color,
    Plane, Ray, Sphere, UeBox, UeBox2d, BoxSphereBounds,
};

// FFI handles (rarely needed directly, but useful for advanced cases)
pub use uika_runtime::{UObjectHandle, UClassHandle, FPropertyHandle, UStructHandle, FNameHandle};

// Proc macros
pub use uika_macros::{uclass, uclass_impl};

// glam re-exports (common math types users will interact with)
pub use glam::{DVec2, DVec3, DVec4, DQuat, DMat4, IVec2, IVec3};

// Core UE types (feature-gated)
#[cfg(feature = "core")]
pub use uika_bindings::core_ue::{
    FVector, FVectorExt,
    FVector2D, FVector2DExt,
    FVector4, FVector4Ext,
    FQuat, FQuatExt,
    FRotator, FRotatorExt,
    FTransform, FTransformExt,
    FLinearColor, FLinearColorExt,
    FColor, FColorExt,
    FPlane, FPlaneExt,
    FBox2D, FBox2DExt,
};

// Manual conversion traits (feature-gated)
#[cfg(feature = "core")]
pub use uika_bindings::manual::vector::OwnedFVectorExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::vector2d::OwnedFVector2DExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::vector4::OwnedFVector4Ext;
#[cfg(feature = "core")]
pub use uika_bindings::manual::quat::OwnedFQuatExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::rotator::OwnedFRotatorExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::transform::OwnedFTransformExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::linear_color::OwnedFLinearColorExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::color::OwnedFColorExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::plane::OwnedFPlaneExt;
#[cfg(feature = "core")]
pub use uika_bindings::manual::ue_box2d::OwnedFBox2DExt;

// Engine types (feature-gated)
#[cfg(feature = "engine")]
pub use uika_bindings::engine::{Actor, ActorExt, World, WorldExt};

// World spawn/query extensions (feature-gated)
#[cfg(feature = "engine")]
pub use uika_bindings::manual::world_ext::{WorldSpawnExt, find_object, load_object};
