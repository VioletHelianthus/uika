/// FFI error codes shared between Rust and C++.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UikaErrorCode {
    Ok = 0,
    ObjectDestroyed = 1,
    InvalidCast = 2,
    PropertyNotFound = 3,
    FunctionNotFound = 4,
    TypeMismatch = 5,
    NullArgument = 6,
    IndexOutOfRange = 7,
    InvalidOperation = 8,
    InternalError = 9,
    BufferTooSmall = 10,
}
