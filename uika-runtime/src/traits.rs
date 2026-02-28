// Marker traits for UE reflection types. Codegen generates impls for all
// exported UClasses, UStructs, and UEnums.

use uika_ffi::{UClassHandle, UObjectHandle, UStructHandle};

use crate::error::UikaResult;

/// Implemented by codegen for every exported UClass.
///
/// Provides the static UClass handle needed for cast checks and reflection
/// queries. The handle is typically cached in a `OnceLock` on first access.
pub trait UeClass: 'static {
    /// Get the UClass* for this type (cached after first call).
    fn static_class() -> UClassHandle;
}

/// Implemented by codegen for every exported UScriptStruct.
pub trait UeStruct: 'static {
    /// Get the UScriptStruct* for this type.
    fn static_struct() -> UStructHandle;
}

/// Implemented by codegen for every exported UEnum.
pub trait UeEnum: 'static {
    /// The underlying integer representation (u8, i32, i64, etc.).
    type Repr: Copy;
}

/// Declares the immediate UE parent class for codegen-exported classes.
///
/// Enables blanket `Deref` impls on `UObjectRef<T>`, `Checked<T>`, and
/// `Pinned<T>` so inherited methods resolve automatically through the
/// Deref chain instead of being flattened into each child's Ext trait.
///
/// Codegen generates `impl HasParent for Pawn { type Parent = Actor; }` etc.
/// Root classes (e.g., `UObject`) do NOT implement this trait.
pub trait HasParent: UeClass {
    type Parent: UeClass;
}

/// Trait for types that hold a UObject handle and can validate it.
///
/// Both `UObjectRef<T>` and `Pinned<T>` implement this, enabling fallible
/// validity checks. For infallible access, use [`ValidHandle`] instead.
pub trait UeHandle {
    /// Return the raw handle if the object is alive, or `Err(ObjectDestroyed)`.
    fn checked_handle(&self) -> UikaResult<UObjectHandle>;

    /// Return the raw handle without validity check.
    fn raw_handle(&self) -> UObjectHandle;
}

/// Trait for types that have been pre-validated and can provide a handle
/// without fallibility. `Checked<T>` and `Pinned<T>` implement this.
///
/// Codegen extension traits use this as a supertrait so that methods
/// return `T` directly instead of `UikaResult<T>`.
pub trait ValidHandle {
    /// Return the raw handle. The implementor guarantees (or debug-asserts)
    /// that the handle is valid.
    fn handle(&self) -> UObjectHandle;
}
