// FName: ergonomic wrapper around FNameHandle.
// Provides construction from &str and Display for string conversion.

use std::fmt;

use uika_ffi::FNameHandle;

use crate::error::check_ffi;
use crate::ffi_dispatch;

/// A UE FName value. Copy-able, hashable, and comparable.
///
/// FName is UE's interned string type — cheap to copy and compare,
/// but creation and string conversion require FFI calls.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FName(pub FNameHandle);

impl FName {
    /// The "None" name (index 0).
    pub const NONE: FName = FName(FNameHandle(0));

    /// Create an FName from a string.
    pub fn new(name: &str) -> Self {
        let handle = unsafe {
            ffi_dispatch::core_make_fname(name.as_ptr(), name.len() as u32)
        };
        FName(handle)
    }

    /// Get the underlying FFI handle.
    #[inline]
    pub fn handle(&self) -> FNameHandle {
        self.0
    }

    /// Check if this is the "None" name.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.0 .0 == 0
    }

    /// Convert to a String. Returns an error only if the FFI call fails.
    pub fn to_string_lossy(&self) -> String {
        // Stack buffer — 256 bytes is enough for virtually all FNames.
        let mut buf = [0u8; 256];
        let mut out_len: u32 = 0;
        let code = unsafe {
            ffi_dispatch::core_fname_to_string(
                self.0,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut out_len,
            )
        };
        if check_ffi(code).is_err() {
            return String::from("<invalid FName>");
        }
        std::str::from_utf8(&buf[..out_len as usize])
            .map(|s| s.to_owned())
            .unwrap_or_else(|_| String::from("<invalid UTF-8>"))
    }
}

impl Default for FName {
    fn default() -> Self {
        FName::NONE
    }
}

impl fmt::Display for FName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_lossy())
    }
}

impl PartialEq<&str> for FName {
    fn eq(&self, other: &&str) -> bool {
        self.to_string_lossy() == *other
    }
}

impl From<&str> for FName {
    fn from(s: &str) -> Self {
        FName::new(s)
    }
}

impl From<FName> for FNameHandle {
    fn from(name: FName) -> FNameHandle {
        name.0
    }
}

impl From<FNameHandle> for FName {
    fn from(handle: FNameHandle) -> FName {
        FName(handle)
    }
}
