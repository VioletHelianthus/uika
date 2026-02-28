// uika-runtime: Safe Rust API wrapping uika-ffi.
// All unsafe FFI calls are confined to this crate. Generated code and user
// code interact only with the safe types exported here.

pub mod api;
pub mod error;
pub mod traits;
pub mod object_ref;
pub mod struct_ref;
pub mod pinned;
pub mod dynamic_call;
pub mod logging;
pub mod ffi_guard;
pub mod containers;
pub mod delegate_registry;
pub mod reify_registry;
pub mod ue_math;
pub mod fname;
pub mod weak_ptr;
pub mod world;

// Re-export the primary public API surface.
pub use api::{api, init_api};
pub use error::{check_ffi, check_ffi_ctx, ffi_infallible, ffi_infallible_ctx, UikaError, UikaResult};
pub use traits::{UeClass, UeStruct, UeEnum, UeHandle, ValidHandle, HasParent};
pub use object_ref::{Checked, UObjectRef};
pub use struct_ref::UStructRef;
pub use pinned::Pinned;
pub use dynamic_call::{DynamicCall, DynamicCallResult};
pub use logging::{LOG_DISPLAY, LOG_WARNING, LOG_ERROR};
pub use ffi_guard::ffi_boundary;
pub use containers::{ContainerElement, OwnedStruct, UeArray, UeMap, UeSet};
pub use delegate_registry::DelegateBinding;

// Phase 10 re-exports.
pub use fname::FName;
pub use weak_ptr::TWeakObjectPtr;
pub use ue_math::{
    Rotator, Transform, LinearColor, Color,
    Plane, Ray, Sphere, UeBox, UeBox2d, BoxSphereBounds,
};

// Re-export FFI types needed by generated code in uika-bindings.
pub use uika_ffi::{
    UObjectHandle, UClassHandle, FPropertyHandle, UStructHandle,
    FNameHandle, FWeakObjectHandle, UikaErrorCode,
};
