// DVec3 ↔ OwnedStruct<FVector> conversions.

use glam::DVec3;
use uika_runtime::{OwnedStruct};

use crate::core_ue::{FVector, FVectorExt};

pub trait OwnedFVectorExt {
    fn to_dvec3(&self) -> DVec3;
}

impl OwnedFVectorExt for OwnedStruct<FVector> {
    fn to_dvec3(&self) -> DVec3 {
        let r = self.as_ref();
        DVec3::new(r.get_x(), r.get_y(), r.get_z())
    }
}

impl FVector {
    pub fn from_dvec3(v: DVec3) -> OwnedStruct<FVector> {
        let s = OwnedStruct::<FVector>::new();
        let r = s.as_ref();
        r.set_x(v.x);
        r.set_y(v.y);
        r.set_z(v.z);
        s
    }
}
