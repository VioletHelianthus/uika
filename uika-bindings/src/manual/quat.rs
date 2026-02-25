// DQuat ↔ OwnedStruct<FQuat> conversions.

use glam::DQuat;
use uika_runtime::{OwnedStruct};

use crate::core_ue::{FQuat, FQuatExt};

pub trait OwnedFQuatExt {
    fn to_dquat(&self) -> DQuat;
}

impl OwnedFQuatExt for OwnedStruct<FQuat> {
    fn to_dquat(&self) -> DQuat {
        let r = self.as_ref();
        DQuat::from_xyzw(r.get_x(), r.get_y(), r.get_z(), r.get_w())
    }
}

impl FQuat {
    pub fn from_dquat(q: DQuat) -> OwnedStruct<FQuat> {
        let s = OwnedStruct::<FQuat>::new();
        let r = s.as_ref();
        r.set_x(q.x);
        r.set_y(q.y);
        r.set_z(q.z);
        r.set_w(q.w);
        s
    }
}
