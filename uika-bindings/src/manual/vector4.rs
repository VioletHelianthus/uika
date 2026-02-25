// DVec4 ↔ OwnedStruct<FVector4> conversions.

use glam::DVec4;
use uika_runtime::{OwnedStruct};

use crate::core_ue::{FVector4, FVector4Ext};

pub trait OwnedFVector4Ext {
    fn to_dvec4(&self) -> DVec4;
}

impl OwnedFVector4Ext for OwnedStruct<FVector4> {
    fn to_dvec4(&self) -> DVec4 {
        let r = self.as_ref();
        DVec4::new(r.get_x(), r.get_y(), r.get_z(), r.get_w())
    }
}

impl FVector4 {
    pub fn from_dvec4(v: DVec4) -> OwnedStruct<FVector4> {
        let s = OwnedStruct::<FVector4>::new();
        let r = s.as_ref();
        r.set_x(v.x);
        r.set_y(v.y);
        r.set_z(v.z);
        r.set_w(v.w);
        s
    }
}
