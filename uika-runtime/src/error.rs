// Error types for the Uika runtime.

use std::fmt;

use uika_ffi::UikaErrorCode;

/// Rich error type for Uika operations.
#[derive(Debug)]
pub enum UikaError {
    ObjectDestroyed,
    InvalidCast,
    PropertyNotFound(String),
    FunctionNotFound(String),
    TypeMismatch,
    NullArgument,
    IndexOutOfRange,
    InvalidOperation(String),
    Internal(String),
    BufferTooSmall,
}

impl fmt::Display for UikaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UikaError::ObjectDestroyed => write!(f, "object has been destroyed"),
            UikaError::InvalidCast => write!(f, "invalid cast"),
            UikaError::PropertyNotFound(name) => write!(f, "property not found: {name}"),
            UikaError::FunctionNotFound(name) => write!(f, "function not found: {name}"),
            UikaError::TypeMismatch => write!(f, "type mismatch"),
            UikaError::NullArgument => write!(f, "null argument"),
            UikaError::IndexOutOfRange => write!(f, "index out of range"),
            UikaError::InvalidOperation(msg) => write!(f, "invalid operation: {msg}"),
            UikaError::Internal(msg) => write!(f, "internal error: {msg}"),
            UikaError::BufferTooSmall => write!(f, "buffer too small"),
        }
    }
}

impl std::error::Error for UikaError {}

/// Convenience alias used throughout the runtime and generated code.
pub type UikaResult<T> = Result<T, UikaError>;

/// Convert an FFI error code to a `UikaResult<()>`.
/// `Ok` maps to `Ok(())`, all others map to the corresponding `UikaError`.
pub fn check_ffi(code: UikaErrorCode) -> UikaResult<()> {
    match code {
        UikaErrorCode::Ok => Ok(()),
        other => Err(UikaError::from(other)),
    }
}

/// Like `check_ffi`, but enriches property/function errors with the given name.
pub fn check_ffi_ctx(code: UikaErrorCode, context: &str) -> UikaResult<()> {
    match code {
        UikaErrorCode::Ok => Ok(()),
        UikaErrorCode::PropertyNotFound => Err(UikaError::PropertyNotFound(context.into())),
        UikaErrorCode::FunctionNotFound => Err(UikaError::FunctionNotFound(context.into())),
        UikaErrorCode::InvalidOperation => Err(UikaError::InvalidOperation(context.into())),
        other => Err(UikaError::from(other)),
    }
}

/// Assert that an FFI call returned `Ok`. Used for codegen-generated methods
/// where handle validation has already been performed and the C++ wrapper
/// is expected to always succeed. Panics in debug builds if the code is not `Ok`.
#[inline(always)]
pub fn ffi_infallible(code: UikaErrorCode) {
    debug_assert_eq!(
        code,
        UikaErrorCode::Ok,
        "FFI call returned {:?} after pre-validation",
        code
    );
}

/// Like [`ffi_infallible`], but includes a context string in the panic message.
#[inline(always)]
pub fn ffi_infallible_ctx(code: UikaErrorCode, ctx: &str) {
    debug_assert_eq!(
        code,
        UikaErrorCode::Ok,
        "FFI '{}' returned {:?} after pre-validation",
        ctx,
        code
    );
}

impl From<UikaErrorCode> for UikaError {
    #[allow(clippy::match_same_arms)]
    fn from(code: UikaErrorCode) -> Self {
        match code {
            UikaErrorCode::Ok => {
                // Callers should not convert Ok into an error. If they do,
                // treat it as an internal logic bug.
                UikaError::Internal("unexpected Ok error code".into())
            }
            UikaErrorCode::ObjectDestroyed => UikaError::ObjectDestroyed,
            UikaErrorCode::InvalidCast => UikaError::InvalidCast,
            UikaErrorCode::PropertyNotFound => UikaError::PropertyNotFound(String::new()),
            UikaErrorCode::FunctionNotFound => UikaError::FunctionNotFound(String::new()),
            UikaErrorCode::TypeMismatch => UikaError::TypeMismatch,
            UikaErrorCode::NullArgument => UikaError::NullArgument,
            UikaErrorCode::IndexOutOfRange => UikaError::IndexOutOfRange,
            UikaErrorCode::InvalidOperation => UikaError::InvalidOperation(String::new()),
            UikaErrorCode::InternalError => UikaError::Internal(String::new()),
            UikaErrorCode::BufferTooSmall => UikaError::BufferTooSmall,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ffi_ok_returns_ok() {
        assert!(check_ffi(UikaErrorCode::Ok).is_ok());
    }

    #[test]
    fn check_ffi_errors_map_correctly() {
        let cases = [
            (UikaErrorCode::ObjectDestroyed, "ObjectDestroyed"),
            (UikaErrorCode::InvalidCast, "InvalidCast"),
            (UikaErrorCode::PropertyNotFound, "PropertyNotFound"),
            (UikaErrorCode::FunctionNotFound, "FunctionNotFound"),
            (UikaErrorCode::TypeMismatch, "TypeMismatch"),
            (UikaErrorCode::NullArgument, "NullArgument"),
            (UikaErrorCode::IndexOutOfRange, "IndexOutOfRange"),
            (UikaErrorCode::InvalidOperation, "InvalidOperation"),
            (UikaErrorCode::InternalError, "Internal"),
        ];
        for (code, expected_variant) in cases {
            let err = check_ffi(code).unwrap_err();
            let debug = format!("{err:?}");
            assert!(
                debug.starts_with(expected_variant),
                "expected {expected_variant}, got {debug}"
            );
        }
    }

    #[test]
    fn display_formats_are_human_readable() {
        let err = UikaError::PropertyNotFound("Health".into());
        assert_eq!(err.to_string(), "property not found: Health");
    }
}
