// UikaDelegateApiImpl.cpp — FUikaDelegateApi implementation.
// Bridges Rust closures to UE delegates (unicast and multicast).

#include "UikaApiTable.h"
#include "UikaDelegateProxy.h"
#include "UObject/UnrealType.h"

#include "UikaModule.h"

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#define UIKA_CHECK_ARGS(ObjHandle, PropHandle)                              \
    UObject* Object = static_cast<UObject*>((ObjHandle).ptr);               \
    if (!Object || !IsValid(Object))                                        \
    {                                                                       \
        return EUikaErrorCode::ObjectDestroyed;                             \
    }                                                                       \
    FProperty* RawProp = static_cast<FProperty*>((PropHandle).ptr);         \
    if (!RawProp)                                                           \
    {                                                                       \
        return EUikaErrorCode::PropertyNotFound;                            \
    }

// ---------------------------------------------------------------------------
// bind_delegate — bind a Rust callback to a unicast delegate
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_BindDelegate(
    UikaUObjectHandle ObjHandle,
    UikaFPropertyHandle PropHandle,
    uint64 CallbackId)
{
    UIKA_CHECK_ARGS(ObjHandle, PropHandle);

    FDelegateProperty* DelegateProp = CastField<FDelegateProperty>(RawProp);
    if (!DelegateProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }

    FScriptDelegate* Delegate = DelegateProp->GetPropertyValuePtr_InContainer(Object);
    if (!Delegate)
    {
        return EUikaErrorCode::InternalError;
    }

    // Create the proxy with Object as outer (lifecycle tied to owner).
    UUikaDelegateProxy* Proxy = NewObject<UUikaDelegateProxy>(Object);
    Proxy->CallbackId = CallbackId;
    Proxy->Signature = DelegateProp->SignatureFunction;
    Proxy->OwnerObject = Object;

    Delegate->BindUFunction(Proxy, UUikaDelegateProxy::FakeFuncName);

    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// unbind_delegate — unbind a unicast delegate
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_UnbindDelegate(
    UikaUObjectHandle ObjHandle,
    UikaFPropertyHandle PropHandle)
{
    UIKA_CHECK_ARGS(ObjHandle, PropHandle);

    FDelegateProperty* DelegateProp = CastField<FDelegateProperty>(RawProp);
    if (!DelegateProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }

    FScriptDelegate* Delegate = DelegateProp->GetPropertyValuePtr_InContainer(Object);
    if (!Delegate)
    {
        return EUikaErrorCode::InternalError;
    }

    Delegate->Unbind();
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// add_multicast — add a Rust callback to a multicast delegate
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_AddMulticast(
    UikaUObjectHandle ObjHandle,
    UikaFPropertyHandle PropHandle,
    uint64 CallbackId)
{
    UIKA_CHECK_ARGS(ObjHandle, PropHandle);

    FMulticastDelegateProperty* MultiProp = CastField<FMulticastDelegateProperty>(RawProp);
    if (!MultiProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }

    // Create the proxy with Object as outer.
    UUikaDelegateProxy* Proxy = NewObject<UUikaDelegateProxy>(Object);
    Proxy->CallbackId = CallbackId;
    Proxy->Signature = MultiProp->SignatureFunction;
    Proxy->OwnerObject = Object;

    // Build a script delegate targeting the proxy.
    FScriptDelegate ScriptDelegate;
    ScriptDelegate.BindUFunction(Proxy, UUikaDelegateProxy::FakeFuncName);

    // AddDelegate works for both Inline and Sparse multicast delegates.
    MultiProp->AddDelegate(MoveTemp(ScriptDelegate), Object);

    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// remove_multicast — remove a Rust callback from a multicast delegate
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_RemoveMulticast(
    UikaUObjectHandle ObjHandle,
    UikaFPropertyHandle PropHandle,
    uint64 CallbackId)
{
    UIKA_CHECK_ARGS(ObjHandle, PropHandle);

    FMulticastDelegateProperty* MultiProp = CastField<FMulticastDelegateProperty>(RawProp);
    if (!MultiProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }

    // Get the invocation list to find our proxy by CallbackId.
    // For inline delegates we can access the invocation list directly.
    // For both types, we iterate to find the matching proxy.
    // Iterate all UUikaDelegateProxy objects owned by this Object to find the matching one.
    TArray<UObject*> Children;
    GetObjectsWithOuter(Object, Children, false);
    for (UObject* Child : Children)
    {
        UUikaDelegateProxy* Proxy = Cast<UUikaDelegateProxy>(Child);
        if (Proxy && Proxy->CallbackId == CallbackId)
        {
            FScriptDelegate ScriptDelegate;
            ScriptDelegate.BindUFunction(Proxy, UUikaDelegateProxy::FakeFuncName);
            MultiProp->RemoveDelegate(ScriptDelegate, Object);
            return EUikaErrorCode::Ok;
        }
    }

    // CallbackId not found — not an error, just means it wasn't bound.
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// broadcast_multicast — trigger a multicast delegate from Rust
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_BroadcastMulticast(
    UikaUObjectHandle ObjHandle,
    UikaFPropertyHandle PropHandle,
    uint8* Params)
{
    UIKA_CHECK_ARGS(ObjHandle, PropHandle);

    FMulticastDelegateProperty* MultiProp = CastField<FMulticastDelegateProperty>(RawProp);
    if (!MultiProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }

    // Use the multicast delegate's built-in broadcast mechanism.
    // This calls ProcessMulticastDelegate which fires all bound delegates.
    // ProcessMulticastDelegate is the ProcessEvent-based broadcast path.
    Object->ProcessEvent(MultiProp->SignatureFunction, Params);

    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Global API struct
// ---------------------------------------------------------------------------

FUikaDelegateApi GDelegateApi = {
    &UikaDelegateApi_BindDelegate,
    &UikaDelegateApi_UnbindDelegate,
    &UikaDelegateApi_AddMulticast,
    &UikaDelegateApi_RemoveMulticast,
    &UikaDelegateApi_BroadcastMulticast,
};
