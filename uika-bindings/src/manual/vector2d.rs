// DVec2 ↔ OwnedStruct<FVector2D> conversions.

use glam::DVec2;
use uika_runtime::{OwnedStruct};

use crate::core_ue::{FVector2D, FVector2DExt};

pub trait OwnedFVector2DExt {
    fn to_dvec2(&self) -> DVec2;
}

impl OwnedFVector2DExt for OwnedStruct<FVector2D> {
    fn to_dvec2(&self) -> DVec2 {
        let r = self.as_ref();
        DVec2::new(r.get_x(), r.get_y())
    }
}

impl FVector2D {
    pub fn from_dvec2(v: DVec2) -> OwnedStruct<FVector2D> {
        let s = OwnedStruct::<FVector2D>::new();
        let r = s.as_ref();
        r.set_x(v.x);
        r.set_y(v.y);
        s
    }
}
