// Manual override conversions: glam/custom types ↔ OwnedStruct<UE type>.
// These provide ergonomic TryFrom/TryInto conversions so users can work
// with glam types and Uika math types instead of raw OwnedStruct.

#[cfg(feature = "core")]
pub mod vector;
#[cfg(feature = "core")]
pub mod vector2d;
#[cfg(feature = "core")]
pub mod vector4;
#[cfg(feature = "core")]
pub mod quat;
#[cfg(feature = "core")]
pub mod rotator;
#[cfg(feature = "core")]
pub mod transform;
#[cfg(feature = "core")]
pub mod linear_color;
#[cfg(feature = "core")]
pub mod color;
#[cfg(feature = "core")]
pub mod plane;
#[cfg(feature = "core")]
pub mod ue_box2d;

#[cfg(feature = "engine")]
pub mod world_ext;
