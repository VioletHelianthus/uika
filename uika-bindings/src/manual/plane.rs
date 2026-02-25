// Plane ↔ OwnedStruct<FPlane> conversions.
// FPlane inherits from FVector (X, Y, Z) and adds W.

use glam::DVec3;
use uika_runtime::{OwnedStruct, Plane};

use crate::core_ue::{FPlane, FPlaneExt, FVector, FVectorExt};

pub trait OwnedFPlaneExt {
    fn to_plane(&self) -> Plane;
}

impl OwnedFPlaneExt for OwnedStruct<FPlane> {
    fn to_plane(&self) -> Plane {
        let r = self.as_ref();
        // Reinterpret the FPlane pointer as FVector to access inherited X/Y/Z.
        let vec_ref = unsafe {
            uika_runtime::UStructRef::<FVector>::from_raw(r.as_ptr().0 as *mut u8)
        };
        let normal = DVec3::new(vec_ref.get_x(), vec_ref.get_y(), vec_ref.get_z());
        let d = r.get_w();
        Plane::new(normal, d)
    }
}

impl FPlane {
    pub fn from_plane(p: Plane) -> OwnedStruct<FPlane> {
        let s = OwnedStruct::<FPlane>::new();
        let r = s.as_ref();
        let vec_ref = unsafe {
            uika_runtime::UStructRef::<FVector>::from_raw(r.as_ptr().0 as *mut u8)
        };
        vec_ref.set_x(p.normal.x);
        vec_ref.set_y(p.normal.y);
        vec_ref.set_z(p.normal.z);
        r.set_w(p.d);
        s
    }
}
