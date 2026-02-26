// UikaPropertyApiImpl.cpp — FUikaPropertyApi implementation.

#include "UikaApiTable.h"
#include "UObject/UnrealType.h"
#include "UObject/TextProperty.h"
#include "UObject/UObjectGlobals.h"

// ---------------------------------------------------------------------------
// Validity macro
// ---------------------------------------------------------------------------

// NOTE: The property API is used for both UObject properties and raw struct
// field access (OwnedStruct buffers). We only null-check the container
// pointer; UObject validity is the caller's responsibility. Using void*
// ensures ContainerPtrToValuePtr uses the offset-only overload (no
// IsValidLowLevel assertion).
#define UIKA_CHECK_VALID(ObjHandle)                                     \
    void* Object = (ObjHandle).ptr;                                     \
    if (!Object)                                                        \
    {                                                                   \
        return EUikaErrorCode::ObjectDestroyed;                         \
    }

// ---------------------------------------------------------------------------
// Bool (bit-field safe via FBoolProperty)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetBoolImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, bool* Out)
{
    UIKA_CHECK_VALID(Obj);
    const FBoolProperty* BoolProp = CastField<FBoolProperty>(static_cast<FProperty*>(Prop.ptr));
    if (!BoolProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    *Out = BoolProp->GetPropertyValue_InContainer(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetBoolImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, bool Val)
{
    UIKA_CHECK_VALID(Obj);
    FBoolProperty* BoolProp = CastField<FBoolProperty>(static_cast<FProperty*>(Prop.ptr));
    if (!BoolProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    BoolProp->SetPropertyValue_InContainer(Object, Val);
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Int32
// ---------------------------------------------------------------------------

static EUikaErrorCode GetI32Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int32* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Out = *Property->ContainerPtrToValuePtr<int32>(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetI32Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int32 Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Property->ContainerPtrToValuePtr<int32>(Object) = Val;
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Int64
// ---------------------------------------------------------------------------

static EUikaErrorCode GetI64Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int64* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Out = *Property->ContainerPtrToValuePtr<int64>(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetI64Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int64 Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Property->ContainerPtrToValuePtr<int64>(Object) = Val;
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// UInt8
// ---------------------------------------------------------------------------

static EUikaErrorCode GetU8Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, uint8* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Out = *Property->ContainerPtrToValuePtr<uint8>(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetU8Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, uint8 Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Property->ContainerPtrToValuePtr<uint8>(Object) = Val;
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Float
// ---------------------------------------------------------------------------

static EUikaErrorCode GetF32Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, float* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Out = *Property->ContainerPtrToValuePtr<float>(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetF32Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, float Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Property->ContainerPtrToValuePtr<float>(Object) = Val;
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Double
// ---------------------------------------------------------------------------

static EUikaErrorCode GetF64Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, double* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Out = *Property->ContainerPtrToValuePtr<double>(Object);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetF64Impl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, double Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    *Property->ContainerPtrToValuePtr<double>(Object) = Val;
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// String (handles FStrProperty and FTextProperty)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetStringImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
                                     uint8* Buf, uint32 BufLen, uint32* OutLen)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);

    FString Value;
    if (const FStrProperty* StrProp = CastField<FStrProperty>(Property))
    {
        Value = StrProp->GetPropertyValue_InContainer(Object);
    }
    else if (const FTextProperty* TextProp = CastField<FTextProperty>(Property))
    {
        Value = TextProp->GetPropertyValue_InContainer(Object).ToString();
    }
    else
    {
        return EUikaErrorCode::TypeMismatch;
    }

    const FTCHARToUTF8 Utf8(*Value);
    const uint32 Len = static_cast<uint32>(Utf8.Length());

    if (OutLen)
    {
        *OutLen = Len;
    }
    if (Buf && BufLen > 0)
    {
        const uint32 CopyLen = FMath::Min(Len, BufLen);
        FMemory::Memcpy(Buf, Utf8.Get(), CopyLen);
    }
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetStringImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
                                     const uint8* InBuf, uint32 Len)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);

    const FString Value(Len, UTF8_TO_TCHAR(reinterpret_cast<const char*>(InBuf)));

    if (FStrProperty* StrProp = CastField<FStrProperty>(Property))
    {
        StrProp->SetPropertyValue_InContainer(Object, Value);
    }
    else if (FTextProperty* TextProp = CastField<FTextProperty>(Property))
    {
        TextProp->SetPropertyValue_InContainer(Object, FText::FromString(Value));
    }
    else
    {
        return EUikaErrorCode::TypeMismatch;
    }
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// FName (stored as opaque uint64)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetFNameImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, UikaFNameHandle* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    const FName* NamePtr = Property->ContainerPtrToValuePtr<FName>(Object);
    static_assert(sizeof(FName) >= sizeof(uint64), "FName must be at least 8 bytes");
    FMemory::Memcpy(&Out->value, NamePtr, sizeof(uint64));
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetFNameImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, UikaFNameHandle Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    FName* NamePtr = Property->ContainerPtrToValuePtr<FName>(Object);
    FMemory::Memcpy(NamePtr, &Val.value, sizeof(uint64));
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Object reference (covers ObjectProperty, ClassProperty, TObjectPtr)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetObjectImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, UikaUObjectHandle* Out)
{
    UIKA_CHECK_VALID(Obj);
    const FObjectPropertyBase* ObjProp = CastField<FObjectPropertyBase>(static_cast<FProperty*>(Prop.ptr));
    if (!ObjProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    UObject* Value = ObjProp->GetObjectPropertyValue_InContainer(Object);
    Out->ptr = Value;
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetObjectImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, UikaUObjectHandle Val)
{
    UIKA_CHECK_VALID(Obj);
    FObjectPropertyBase* ObjProp = CastField<FObjectPropertyBase>(static_cast<FProperty*>(Prop.ptr));
    if (!ObjProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    ObjProp->SetObjectPropertyValue_InContainer(Object, static_cast<UObject*>(Val.ptr));
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Enum (handles FEnumProperty and enum-backed FByteProperty)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetEnumImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int64* Out)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);

    if (const FEnumProperty* EnumProp = CastField<FEnumProperty>(Property))
    {
        const FNumericProperty* UnderlyingProp = EnumProp->GetUnderlyingProperty();
        const void* ValuePtr = EnumProp->ContainerPtrToValuePtr<void>(Object);
        *Out = UnderlyingProp->GetSignedIntPropertyValue(ValuePtr);
    }
    else if (const FByteProperty* ByteProp = CastField<FByteProperty>(Property))
    {
        *Out = static_cast<int64>(*ByteProp->ContainerPtrToValuePtr<uint8>(Object));
    }
    else
    {
        return EUikaErrorCode::TypeMismatch;
    }
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetEnumImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop, int64 Val)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);

    if (const FEnumProperty* EnumProp = CastField<FEnumProperty>(Property))
    {
        FNumericProperty* UnderlyingProp = const_cast<FNumericProperty*>(EnumProp->GetUnderlyingProperty());
        void* ValuePtr = EnumProp->ContainerPtrToValuePtr<void>(Object);
        UnderlyingProp->SetIntPropertyValue(ValuePtr, Val);
    }
    else if (FByteProperty* ByteProp = CastField<FByteProperty>(Property))
    {
        *ByteProp->ContainerPtrToValuePtr<uint8>(Object) = static_cast<uint8>(Val);
    }
    else
    {
        return EUikaErrorCode::TypeMismatch;
    }
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Struct (raw memory copy via FStructProperty)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetStructImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
                                     uint8* OutBuf, uint32 BufSize)
{
    UIKA_CHECK_VALID(Obj);
    const FStructProperty* StructProp = CastField<FStructProperty>(static_cast<FProperty*>(Prop.ptr));
    if (!StructProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    const void* SrcPtr = StructProp->ContainerPtrToValuePtr<void>(Object);
    StructProp->Struct->CopyScriptStruct(OutBuf, SrcPtr);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetStructImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
                                     const uint8* InBuf, uint32 BufSize)
{
    UIKA_CHECK_VALID(Obj);
    const FStructProperty* StructProp = CastField<FStructProperty>(static_cast<FProperty*>(Prop.ptr));
    if (!StructProp)
    {
        return EUikaErrorCode::TypeMismatch;
    }
    void* DstPtr = StructProp->ContainerPtrToValuePtr<void>(Object);
    StructProp->Struct->CopyScriptStruct(DstPtr, InBuf);
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Indexed property access (fixed arrays with array_dim > 1)
// ---------------------------------------------------------------------------

static EUikaErrorCode GetPropertyAtImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
    uint32 Index, uint8* OutBuf, uint32 BufSize)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    if (!Property) return EUikaErrorCode::PropertyNotFound;
    if (Index >= (uint32)Property->ArrayDim) return EUikaErrorCode::IndexOutOfRange;

    const void* Src = Property->ContainerPtrToValuePtr<void>(Object, Index);
    uint32 ElemSize = Property->GetElementSize();
    if (BufSize < ElemSize) return EUikaErrorCode::InternalError;

    Property->CopySingleValue(OutBuf, Src);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode SetPropertyAtImpl(UikaUObjectHandle Obj, UikaFPropertyHandle Prop,
    uint32 Index, const uint8* InBuf, uint32 BufSize)
{
    UIKA_CHECK_VALID(Obj);
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    if (!Property) return EUikaErrorCode::PropertyNotFound;
    if (Index >= (uint32)Property->ArrayDim) return EUikaErrorCode::IndexOutOfRange;

    void* Dest = Property->ContainerPtrToValuePtr<void>(Object, Index);
    uint32 ElemSize = Property->GetElementSize();
    if (BufSize < ElemSize) return EUikaErrorCode::InternalError;

    Property->CopySingleValue(Dest, InBuf);
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Static instance
// ---------------------------------------------------------------------------

FUikaPropertyApi GPropertyApi = {
    // Bool
    &GetBoolImpl,
    &SetBoolImpl,
    // Int32
    &GetI32Impl,
    &SetI32Impl,
    // Int64
    &GetI64Impl,
    &SetI64Impl,
    // UInt8
    &GetU8Impl,
    &SetU8Impl,
    // Float
    &GetF32Impl,
    &SetF32Impl,
    // Double
    &GetF64Impl,
    &SetF64Impl,
    // String
    &GetStringImpl,
    &SetStringImpl,
    // FName
    &GetFNameImpl,
    &SetFNameImpl,
    // Object
    &GetObjectImpl,
    &SetObjectImpl,
    // Enum
    &GetEnumImpl,
    &SetEnumImpl,
    // Struct
    &GetStructImpl,
    &SetStructImpl,
    // Indexed access (fixed arrays)
    &GetPropertyAtImpl,
    &SetPropertyAtImpl,
};
