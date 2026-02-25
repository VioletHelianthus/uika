// Rotator ↔ OwnedStruct<FRotator> conversions.

use uika_runtime::{OwnedStruct, Rotator};

use crate::core_ue::{FRotator, FRotatorExt};

pub trait OwnedFRotatorExt {
    fn to_rotator(&self) -> Rotator;
}

impl OwnedFRotatorExt for OwnedStruct<FRotator> {
    fn to_rotator(&self) -> Rotator {
        let r = self.as_ref();
        Rotator::new(r.get_pitch(), r.get_yaw(), r.get_roll())
    }
}

impl FRotator {
    pub fn from_rotator(rot: Rotator) -> OwnedStruct<FRotator> {
        let s = OwnedStruct::<FRotator>::new();
        let r = s.as_ref();
        r.set_pitch(rot.pitch);
        r.set_yaw(rot.yaw);
        r.set_roll(rot.roll);
        s
    }
}
