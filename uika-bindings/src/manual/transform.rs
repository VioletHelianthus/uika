// Transform ↔ OwnedStruct<FTransform> conversions (nested struct fields).

use uika_runtime::{OwnedStruct, Transform};

use crate::core_ue::{FTransform, FTransformExt};
use super::quat::OwnedFQuatExt;
use super::vector::OwnedFVectorExt;

pub trait OwnedFTransformExt {
    fn to_transform(&self) -> Transform;
}

impl OwnedFTransformExt for OwnedStruct<FTransform> {
    fn to_transform(&self) -> Transform {
        let r = self.as_ref();

        let rotation = r.get_rotation().to_dquat();
        let translation = r.get_translation().to_dvec3();
        let scale = r.get_scale3_d().to_dvec3();

        Transform::new(rotation, translation, scale)
    }
}

impl FTransform {
    pub fn from_transform(t: Transform) -> OwnedStruct<FTransform> {
        use crate::core_ue::{FQuat, FVector};

        let s = OwnedStruct::<FTransform>::new();
        let r = s.as_ref();

        let rotation = FQuat::from_dquat(t.rotation);
        r.set_rotation(&rotation);

        let translation = FVector::from_dvec3(t.translation);
        r.set_translation(&translation);

        let scale = FVector::from_dvec3(t.scale);
        r.set_scale3_d(&scale);

        s
    }
}
