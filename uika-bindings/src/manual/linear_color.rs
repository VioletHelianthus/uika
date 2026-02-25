// LinearColor ↔ OwnedStruct<FLinearColor> conversions.

use uika_runtime::{LinearColor, OwnedStruct};

use crate::core_ue::{FLinearColor, FLinearColorExt};

pub trait OwnedFLinearColorExt {
    fn to_linear_color(&self) -> LinearColor;
}

impl OwnedFLinearColorExt for OwnedStruct<FLinearColor> {
    fn to_linear_color(&self) -> LinearColor {
        let r = self.as_ref();
        LinearColor::new(r.get_r(), r.get_g(), r.get_b(), r.get_a())
    }
}

impl FLinearColor {
    pub fn from_linear_color(c: LinearColor) -> OwnedStruct<FLinearColor> {
        let s = OwnedStruct::<FLinearColor>::new();
        let r = s.as_ref();
        r.set_r(c.r);
        r.set_g(c.g);
        r.set_b(c.b);
        r.set_a(c.a);
        s
    }
}
