// UeBox2d ↔ OwnedStruct<FBox2D> conversions (nested FVector2D fields).

use uika_runtime::{OwnedStruct, UeBox2d};

use crate::core_ue::{FBox2D, FBox2DExt, FVector2D};
use super::vector2d::OwnedFVector2DExt;

pub trait OwnedFBox2DExt {
    fn to_ue_box2d(&self) -> UeBox2d;
}

impl OwnedFBox2DExt for OwnedStruct<FBox2D> {
    fn to_ue_box2d(&self) -> UeBox2d {
        let r = self.as_ref();
        let min = r.get_min().to_dvec2();
        let max = r.get_max().to_dvec2();
        UeBox2d::new(min, max)
    }
}

impl FBox2D {
    pub fn from_ue_box2d(b: UeBox2d) -> OwnedStruct<FBox2D> {
        let s = OwnedStruct::<FBox2D>::new();
        let r = s.as_ref();
        let min = FVector2D::from_dvec2(b.min);
        r.set_min(&min);
        let max = FVector2D::from_dvec2(b.max);
        r.set_max(&max);
        s
    }
}
