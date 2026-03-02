// FKey: UE input key identifier, wrapping an FName.
// Provides a distinct Rust type matching the UE FKey struct.

use std::fmt;

use uika_ffi::FNameHandle;

use crate::FName;

/// A UE input key identifier (`FKey`).
///
/// FKey's identity is determined solely by its key name (an FName).
/// Construct with `FKey::new("LeftMouseButton")` or convert from an FName.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(transparent)]
pub struct FKey(pub FNameHandle);

impl FKey {
    /// Create an FKey from a key name string.
    ///
    /// Key names follow UE conventions: `"LeftMouseButton"`, `"RightMouseButton"`,
    /// `"SpaceBar"`, `"W"`, `"Gamepad_LeftX"`, etc.
    pub fn new(key_name: &str) -> Self {
        FKey(FName::new(key_name).handle())
    }

    /// Get the key name as an FName.
    pub fn key_name(&self) -> FName {
        FName(self.0)
    }

    /// Get the underlying FFI handle.
    #[inline]
    pub fn handle(&self) -> FNameHandle {
        self.0
    }
}

impl fmt::Display for FKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key_name())
    }
}

impl From<FName> for FKey {
    fn from(name: FName) -> Self {
        FKey(name.handle())
    }
}

impl From<FKey> for FName {
    fn from(key: FKey) -> Self {
        FName(key.0)
    }
}

impl From<FNameHandle> for FKey {
    fn from(handle: FNameHandle) -> Self {
        FKey(handle)
    }
}

impl From<FKey> for FNameHandle {
    fn from(key: FKey) -> Self {
        key.0
    }
}
