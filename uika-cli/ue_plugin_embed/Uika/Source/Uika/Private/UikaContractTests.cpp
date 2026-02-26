// UikaContractTests.cpp — Compile-time FFI contract tests.
// These static_asserts ensure C++ handle types and API structs
// are layout-compatible with their Rust #[repr(C)] counterparts.

#include "UikaApiTable.h"

// ---------------------------------------------------------------------------
// Handle sizes (must match Rust side exactly)
// ---------------------------------------------------------------------------

static_assert(sizeof(UikaUObjectHandle)      == 8,  "UikaUObjectHandle must be 8 bytes");
static_assert(sizeof(UikaUClassHandle)       == 8,  "UikaUClassHandle must be 8 bytes");
static_assert(sizeof(UikaFPropertyHandle)    == 8,  "UikaFPropertyHandle must be 8 bytes");
static_assert(sizeof(UikaUFunctionHandle)    == 8,  "UikaUFunctionHandle must be 8 bytes");
static_assert(sizeof(UikaUStructHandle)      == 8,  "UikaUStructHandle must be 8 bytes");
static_assert(sizeof(UikaFNameHandle)        == 8,  "UikaFNameHandle must be 8 bytes");
static_assert(sizeof(UikaFWeakObjectHandle)  == 8,  "UikaFWeakObjectHandle must be 8 bytes");

// ---------------------------------------------------------------------------
// Error code size
// ---------------------------------------------------------------------------

static_assert(sizeof(EUikaErrorCode) == 4, "EUikaErrorCode must be 4 bytes (uint32)");

// ---------------------------------------------------------------------------
// Handle alignment
// ---------------------------------------------------------------------------

static_assert(alignof(UikaUObjectHandle)     == alignof(void*), "UikaUObjectHandle alignment");
static_assert(alignof(UikaUClassHandle)      == alignof(void*), "UikaUClassHandle alignment");
static_assert(alignof(UikaFPropertyHandle)   == alignof(void*), "UikaFPropertyHandle alignment");
static_assert(alignof(UikaUFunctionHandle)   == alignof(void*), "UikaUFunctionHandle alignment");
static_assert(alignof(UikaUStructHandle)     == alignof(void*), "UikaUStructHandle alignment");
static_assert(alignof(UikaFNameHandle)       == alignof(uint64), "UikaFNameHandle alignment");

// ---------------------------------------------------------------------------
// Weak object handle layout
// ---------------------------------------------------------------------------

static_assert(offsetof(UikaFWeakObjectHandle, object_index)         == 0, "FWeakObjectHandle::object_index at offset 0");
static_assert(offsetof(UikaFWeakObjectHandle, object_serial_number) == 4, "FWeakObjectHandle::object_serial_number at offset 4");
