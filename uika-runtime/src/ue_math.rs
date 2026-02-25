// UE math types without direct glam equivalents.
// These are simple Rust structs with conversions to/from glam types where applicable.

use glam::{DQuat, DVec2, DVec3, Vec4};

// ---------------------------------------------------------------------------
// Rotator (FRotator equivalent — pitch/yaw/roll in degrees)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotator {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

impl Rotator {
    pub const ZERO: Rotator = Rotator { pitch: 0.0, yaw: 0.0, roll: 0.0 };

    pub fn new(pitch: f64, yaw: f64, roll: f64) -> Self {
        Rotator { pitch, yaw, roll }
    }
}

// UE uses intrinsic ZYX rotation order (Yaw → Pitch → Roll), angles in degrees.
// UE axes: Pitch=Y, Yaw=Z, Roll=X.
impl From<Rotator> for DQuat {
    fn from(r: Rotator) -> DQuat {
        let deg2rad = std::f64::consts::PI / 180.0;
        let (sp, cp) = (r.pitch * 0.5 * deg2rad).sin_cos();
        let (sy, cy) = (r.yaw * 0.5 * deg2rad).sin_cos();
        let (sr, cr) = (r.roll * 0.5 * deg2rad).sin_cos();

        // Standard ZYX: Quat = Qz(yaw) * Qy(pitch) * Qx(roll)
        DQuat::from_xyzw(
            cy * cp * sr - sy * sp * cr,
            cy * sp * cr + sy * cp * sr,
            sy * cp * cr - cy * sp * sr,
            cy * cp * cr + sy * sp * sr,
        )
    }
}

impl From<DQuat> for Rotator {
    fn from(q: DQuat) -> Rotator {
        let rad2deg = 180.0 / std::f64::consts::PI;

        // Extract Euler angles (UE convention: intrinsic ZYX → extrinsic XYZ)
        let sinr_cosp = 2.0 * (q.w * q.x + q.y * q.z);
        let cosr_cosp = 1.0 - 2.0 * (q.x * q.x + q.y * q.y);
        let roll = sinr_cosp.atan2(cosr_cosp);

        let sinp = 2.0 * (q.w * q.y - q.z * q.x);
        let pitch = if sinp.abs() >= 1.0 {
            std::f64::consts::FRAC_PI_2.copysign(sinp)
        } else {
            sinp.asin()
        };

        let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
        let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
        let yaw = siny_cosp.atan2(cosy_cosp);

        Rotator {
            pitch: pitch * rad2deg,
            yaw: yaw * rad2deg,
            roll: roll * rad2deg,
        }
    }
}

// ---------------------------------------------------------------------------
// Transform (FTransform equivalent)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub rotation: DQuat,
    pub translation: DVec3,
    pub scale: DVec3,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        rotation: DQuat::IDENTITY,
        translation: DVec3::ZERO,
        scale: DVec3::ONE,
    };

    pub fn new(rotation: DQuat, translation: DVec3, scale: DVec3) -> Self {
        Transform { rotation, translation, scale }
    }

    pub fn from_translation(translation: DVec3) -> Self {
        Transform { translation, ..Self::IDENTITY }
    }

    pub fn from_rotation(rotation: DQuat) -> Self {
        Transform { rotation, ..Self::IDENTITY }
    }
}

// ---------------------------------------------------------------------------
// Color types
// ---------------------------------------------------------------------------

/// Linear color (float RGBA, 0.0–1.0 range). Maps to FLinearColor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl LinearColor {
    pub const BLACK: LinearColor = LinearColor { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: LinearColor = LinearColor { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const RED: LinearColor = LinearColor { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: LinearColor = LinearColor { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: LinearColor = LinearColor { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        LinearColor { r, g, b, a }
    }
}

impl From<LinearColor> for Vec4 {
    fn from(c: LinearColor) -> Vec4 {
        Vec4::new(c.r, c.g, c.b, c.a)
    }
}

impl From<Vec4> for LinearColor {
    fn from(v: Vec4) -> LinearColor {
        LinearColor { r: v.x, g: v.y, b: v.z, a: v.w }
    }
}

/// 8-bit RGBA color. Maps to FColor (note: UE stores BGRA internally,
/// conversions handle the reorder).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Color = Color { r: 255, g: 0, b: 0, a: 255 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0, a: 255 };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255, a: 255 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
}

// ---------------------------------------------------------------------------
// Geometry primitives
// ---------------------------------------------------------------------------

/// A plane defined by normal + distance from origin. Maps to FPlane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: DVec3,
    pub d: f64,
}

impl Plane {
    pub fn new(normal: DVec3, d: f64) -> Self {
        Plane { normal, d }
    }
}

/// A ray defined by origin + direction. Maps to FRay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: DVec3,
    pub direction: DVec3,
}

impl Ray {
    pub fn new(origin: DVec3, direction: DVec3) -> Self {
        Ray { origin, direction }
    }
}

/// A sphere defined by center + radius. Maps to FSphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: DVec3,
    pub radius: f64,
}

impl Sphere {
    pub fn new(center: DVec3, radius: f64) -> Self {
        Sphere { center, radius }
    }
}

// ---------------------------------------------------------------------------
// Bounding volumes
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box. Named `UeBox` to avoid conflict with Rust's `Box`.
/// Maps to FBox.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UeBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl UeBox {
    pub fn new(min: DVec3, max: DVec3) -> Self {
        UeBox { min, max }
    }
}

/// 2D axis-aligned bounding box. Maps to FBox2D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UeBox2d {
    pub min: DVec2,
    pub max: DVec2,
}

impl UeBox2d {
    pub fn new(min: DVec2, max: DVec2) -> Self {
        UeBox2d { min, max }
    }
}

/// Combined box + sphere bounds. Maps to FBoxSphereBounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxSphereBounds {
    pub origin: DVec3,
    pub box_extent: DVec3,
    pub sphere_radius: f64,
}

impl BoxSphereBounds {
    pub fn new(origin: DVec3, box_extent: DVec3, sphere_radius: f64) -> Self {
        BoxSphereBounds { origin, box_extent, sphere_radius }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotator_zero_to_quat_is_identity() {
        let q: DQuat = Rotator::ZERO.into();
        let diff = (q - DQuat::IDENTITY).length();
        assert!(diff < 1e-10, "Expected identity quat, got {q:?}");
    }

    #[test]
    fn rotator_roundtrip() {
        let r = Rotator::new(30.0, 45.0, 60.0);
        let q: DQuat = r.into();
        let r2: Rotator = q.into();
        assert!((r.pitch - r2.pitch).abs() < 1e-10);
        assert!((r.yaw - r2.yaw).abs() < 1e-10);
        assert!((r.roll - r2.roll).abs() < 1e-10);
    }

    #[test]
    fn linear_color_vec4_roundtrip() {
        let c = LinearColor::new(0.5, 0.3, 0.8, 1.0);
        let v: Vec4 = c.into();
        let c2: LinearColor = v.into();
        assert_eq!(c, c2);
    }
}
