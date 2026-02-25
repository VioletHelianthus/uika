// Color ↔ OwnedStruct<FColor> conversions.

use uika_runtime::{Color, OwnedStruct};

use crate::core_ue::{FColor, FColorExt};

pub trait OwnedFColorExt {
    fn to_color(&self) -> Color;
}

impl OwnedFColorExt for OwnedStruct<FColor> {
    fn to_color(&self) -> Color {
        let r = self.as_ref();
        Color::new(r.get_r(), r.get_g(), r.get_b(), r.get_a())
    }
}

impl FColor {
    pub fn from_color(c: Color) -> OwnedStruct<FColor> {
        let s = OwnedStruct::<FColor>::new();
        let r = s.as_ref();
        r.set_r(c.r);
        r.set_g(c.g);
        r.set_b(c.b);
        r.set_a(c.a);
        s
    }
}
