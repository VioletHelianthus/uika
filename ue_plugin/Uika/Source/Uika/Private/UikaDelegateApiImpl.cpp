// UikaDelegateApiImpl.cpp — FUikaDelegateApi implementation.
// Bridges Rust closures to UE delegates (unicast and multicast).

#include "UikaApiTable.h"
#include "UikaDelegateProxy.h"
#include "UikaFNameHelper.h"
#include "UObject/UnrealType.h"
#include "UObject/TextProperty.h"

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
// read_param — read a typed parameter from a raw ProcessEvent params buffer
// ---------------------------------------------------------------------------

static EUikaErrorCode UikaDelegateApi_ReadParam(
    UikaFPropertyHandle PropHandle,
    void* ParamsBuf,
    uint32 Offset,
    uint8* OutBuf,
    uint32 OutBufSize,
    uint32* OutWritten)
{
    FProperty* Prop = static_cast<FProperty*>(PropHandle.ptr);
    if (!Prop || !ParamsBuf)
    {
        return EUikaErrorCode::NullArgument;
    }

    const void* ValuePtr = static_cast<const uint8*>(ParamsBuf) + Offset;

    // String (FString)
    if (const FStrProperty* StrProp = CastField<FStrProperty>(Prop))
    {
        const FString& Str = StrProp->GetPropertyValue(ValuePtr);
        FTCHARToUTF8 Utf8(*Str);
        uint32 Len = static_cast<uint32>(Utf8.Length());
        uint32 Required = sizeof(uint32) + Len;
        if (OutWritten) *OutWritten = Required;
        if (OutBufSize < Required)
        {
            return EUikaErrorCode::BufferTooSmall;
        }
        FMemory::Memcpy(OutBuf, &Len, sizeof(uint32));
        FMemory::Memcpy(OutBuf + sizeof(uint32), Utf8.Get(), Len);
        return EUikaErrorCode::Ok;
    }

    // Text (FText -> FString)
    if (const FTextProperty* TextProp = CastField<FTextProperty>(Prop))
    {
        FString Str = TextProp->GetPropertyValue(ValuePtr).ToString();
        FTCHARToUTF8 Utf8(*Str);
        uint32 Len = static_cast<uint32>(Utf8.Length());
        uint32 Required = sizeof(uint32) + Len;
        if (OutWritten) *OutWritten = Required;
        if (OutBufSize < Required)
        {
            return EUikaErrorCode::BufferTooSmall;
        }
        FMemory::Memcpy(OutBuf, &Len, sizeof(uint32));
        FMemory::Memcpy(OutBuf + sizeof(uint32), Utf8.Get(), Len);
        return EUikaErrorCode::Ok;
    }

    // FName -> pack into uint64
    if (CastField<FNameProperty>(Prop))
    {
        const FName* NamePtr = static_cast<const FName*>(ValuePtr);
        uint64 Packed = UikaPackFName(*NamePtr);
        if (OutWritten) *OutWritten = sizeof(uint64);
        if (OutBufSize < sizeof(uint64))
        {
            return EUikaErrorCode::BufferTooSmall;
        }
        FMemory::Memcpy(OutBuf, &Packed, sizeof(uint64));
        return EUikaErrorCode::Ok;
    }

    // Struct -> CopyScriptStruct
    if (const FStructProperty* StructProp = CastField<FStructProperty>(Prop))
    {
        uint32 Size = StructProp->GetSize();
        if (OutWritten) *OutWritten = Size;
        if (OutBufSize < Size)
        {
            return EUikaErrorCode::BufferTooSmall;
        }
        StructProp->Struct->CopyScriptStruct(OutBuf, ValuePtr);
        return EUikaErrorCode::Ok;
    }

    // Object
    if (const FObjectPropertyBase* ObjProp = CastField<FObjectPropertyBase>(Prop))
    {
        UObject* Obj = ObjProp->GetObjectPropertyValue(ValuePtr);
        if (OutWritten) *OutWritten = sizeof(void*);
        if (OutBufSize < sizeof(void*))
        {
            return EUikaErrorCode::BufferTooSmall;
        }
        FMemory::Memcpy(OutBuf, &Obj, sizeof(void*));
        return EUikaErrorCode::Ok;
    }

    // Fallback: raw memcpy (primitives, enums)
    uint32 Size = Prop->GetSize();
    if (OutWritten) *OutWritten = Size;
    if (OutBufSize < Size)
    {
        return EUikaErrorCode::BufferTooSmall;
    }
    FMemory::Memcpy(OutBuf, ValuePtr, Size);
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
    &UikaDelegateApi_ReadParam,
};
