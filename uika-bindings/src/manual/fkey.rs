// FKey construction and access helpers.
//
// FKey is an opaque UE struct (FName + TSharedPtr<FKeyDetails>).
// Codegen generates a marker type with UeStruct, but FKey has no UPROPERTY
// members, so we provide manual helpers for name access and construction.

use uika_runtime::{FName, FNameHandle, OwnedStruct, UStructRef};

use crate::input_core::FKey;

/// FKey access helpers via UStructRef.
///
/// FName is at offset 0 within FKey (stable since UE4).
pub trait FKeyExt {
    /// Get the key name as an FName.
    fn key_name(&self) -> FName;
    /// Check if this key matches a given key name string.
    fn is_key(&self, name: &str) -> bool;
}

impl FKeyExt for UStructRef<FKey> {
    fn key_name(&self) -> FName {
        unsafe {
            let ptr = self.as_ptr().0 as *const u8;
            FName(FNameHandle(*(ptr as *const u64)))
        }
    }

    fn is_key(&self, name: &str) -> bool {
        self.key_name() == FName::new(name)
    }
}

impl FKey {
    /// Create an OwnedStruct<FKey> from a key name string (correct UE memory size).
    ///
    /// Key names follow UE conventions: `"LeftMouseButton"`, `"RightMouseButton"`,
    /// `"SpaceBar"`, `"W"`, `"Gamepad_LeftX"`, etc.
    pub fn named(key_name: &str) -> OwnedStruct<FKey> {
        let s = OwnedStruct::<FKey>::new();
        unsafe {
            let ptr = s.as_ref().as_ptr().0 as *mut u8;
            *(ptr as *mut u64) = FName::new(key_name).handle().0;
        }
        s
    }
}
