// UikaReflectionApiImpl.cpp — FUikaReflectionApi implementation.

#include "UikaApiTable.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UnrealType.h"

// Helper: convert UTF-8 byte slice to FName.
static FName Utf8ToFName(const uint8* Name, uint32 NameLen)
{
    const FString Str(NameLen, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Name)));
    return FName(*Str);
}

// Helper: convert UTF-8 byte slice to FString.
static FString Utf8ToFString(const uint8* Name, uint32 NameLen)
{
    return FString(NameLen, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Name)));
}

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

static UikaUClassHandle FindClassImpl(const uint8* Name, uint32 NameLen)
{
    const FString ClassName = Utf8ToFString(Name, NameLen);
    UClass* Found = FindFirstObject<UClass>(*ClassName, EFindFirstObjectOptions::NativeFirst);
    return UikaUClassHandle{ Found };
}

static UikaFPropertyHandle FindPropertyImpl(UikaUClassHandle Cls, const uint8* Name, uint32 NameLen)
{
    UClass* Class = static_cast<UClass*>(Cls.ptr);
    if (!Class)
    {
        return UikaFPropertyHandle{ nullptr };
    }
    const FName PropName = Utf8ToFName(Name, NameLen);
    FProperty* Prop = Class->FindPropertyByName(PropName);
    return UikaFPropertyHandle{ Prop };
}

static UikaUClassHandle GetStaticClassImpl(const uint8* Name, uint32 NameLen)
{
    // Same as FindClass — generated code passes the class short name.
    return FindClassImpl(Name, NameLen);
}

static uint32 GetPropertySizeImpl(UikaFPropertyHandle Prop)
{
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    if (!Property)
    {
        return 0;
    }
    return static_cast<uint32>(Property->GetSize());
}

static UikaUStructHandle FindStructImpl(const uint8* Name, uint32 NameLen)
{
    const FString StructName = Utf8ToFString(Name, NameLen);
    UScriptStruct* Found = FindFirstObject<UScriptStruct>(*StructName, EFindFirstObjectOptions::NativeFirst);
    return UikaUStructHandle{ Found };
}

static UikaFPropertyHandle FindStructPropertyImpl(UikaUStructHandle UStruct, const uint8* Name, uint32 NameLen)
{
    UScriptStruct* Struct = static_cast<UScriptStruct*>(UStruct.ptr);
    if (!Struct)
    {
        return UikaFPropertyHandle{ nullptr };
    }
    const FName PropName = Utf8ToFName(Name, NameLen);
    FProperty* Prop = Struct->FindPropertyByName(PropName);
    return UikaFPropertyHandle{ Prop };
}

// ---------------------------------------------------------------------------
// Dynamic call implementations (Phase 6)
// ---------------------------------------------------------------------------

static UikaUFunctionHandle FindFunctionImpl(UikaUObjectHandle Obj, const uint8* Name, uint32 NameLen)
{
    UObject* Object = static_cast<UObject*>(Obj.ptr);
    if (!::IsValid(Object)) return UikaUFunctionHandle{ nullptr };
    const FName FuncName = Utf8ToFName(Name, NameLen);
    UFunction* Func = Object->FindFunction(FuncName);
    return UikaUFunctionHandle{ Func };
}

static uint8* AllocParamsImpl(UikaUFunctionHandle Func)
{
    UFunction* Function = static_cast<UFunction*>(Func.ptr);
    if (!Function || Function->ParmsSize == 0) return nullptr;
    uint8* Params = static_cast<uint8*>(FMemory::Malloc(Function->ParmsSize));
    FMemory::Memzero(Params, Function->ParmsSize);
    // Initialize properties to defaults
    for (TFieldIterator<FProperty> It(Function); It && It->HasAnyPropertyFlags(CPF_Parm); ++It)
    {
        It->InitializeValue_InContainer(Params);
    }
    return Params;
}

static void FreeParamsImpl(UikaUFunctionHandle Func, uint8* Params)
{
    if (!Params) return;
    UFunction* Function = static_cast<UFunction*>(Func.ptr);
    if (Function)
    {
        for (TFieldIterator<FProperty> It(Function); It && It->HasAnyPropertyFlags(CPF_Parm); ++It)
        {
            It->DestroyValue_InContainer(Params);
        }
    }
    FMemory::Free(Params);
}

static EUikaErrorCode CallFunctionImpl(UikaUObjectHandle Obj, UikaUFunctionHandle Func, uint8* Params)
{
    UObject* Object = static_cast<UObject*>(Obj.ptr);
    if (!::IsValid(Object)) return EUikaErrorCode::ObjectDestroyed;
    UFunction* Function = static_cast<UFunction*>(Func.ptr);
    if (!Function) return EUikaErrorCode::FunctionNotFound;
    Object->ProcessEvent(Function, Params);
    return EUikaErrorCode::Ok;
}

static UikaFPropertyHandle GetFunctionParamImpl(UikaUFunctionHandle Func, const uint8* Name, uint32 NameLen)
{
    UFunction* Function = static_cast<UFunction*>(Func.ptr);
    if (!Function) return UikaFPropertyHandle{ nullptr };
    const FName PropName = Utf8ToFName(Name, NameLen);
    FProperty* Prop = Function->FindPropertyByName(PropName);
    return UikaFPropertyHandle{ Prop };
}

static uint32 GetPropertyOffsetImpl(UikaFPropertyHandle Prop)
{
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    if (!Property) return 0;
    return static_cast<uint32>(Property->GetOffset_ForUFunction());
}

static UikaUFunctionHandle FindFunctionByClassImpl(UikaUClassHandle Cls, const uint8* Name, uint32 NameLen)
{
    UClass* Class = static_cast<UClass*>(Cls.ptr);
    if (!Class) return UikaUFunctionHandle{ nullptr };
    const FName FuncName = Utf8ToFName(Name, NameLen);
    UFunction* Func = Class->FindFunctionByName(FuncName);
    return UikaUFunctionHandle{ Func };
}

static uint32 GetElementSizeImpl(UikaFPropertyHandle Prop)
{
    FProperty* Property = static_cast<FProperty*>(Prop.ptr);
    return Property ? Property->GetElementSize() : 0;
}

static uint32 GetStructSizeImpl(UikaUStructHandle UStruct)
{
    UScriptStruct* Struct = static_cast<UScriptStruct*>(UStruct.ptr);
    return Struct ? static_cast<uint32>(Struct->GetStructureSize()) : 0;
}

static EUikaErrorCode InitializeStructImpl(UikaUStructHandle UStructHandle, uint8* Data)
{
    UScriptStruct* Struct = static_cast<UScriptStruct*>(UStructHandle.ptr);
    if (!Struct || !Data) return EUikaErrorCode::NullArgument;
    Struct->InitializeStruct(Data);
    return EUikaErrorCode::Ok;
}

static EUikaErrorCode DestroyStructImpl(UikaUStructHandle UStructHandle, uint8* Data)
{
    UScriptStruct* Struct = static_cast<UScriptStruct*>(UStructHandle.ptr);
    if (!Struct || !Data) return EUikaErrorCode::NullArgument;
    Struct->DestroyStruct(Data);
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Static instance
// ---------------------------------------------------------------------------

FUikaReflectionApi GReflectionApi = {
    &FindClassImpl,
    &FindPropertyImpl,
    &GetStaticClassImpl,
    &GetPropertySizeImpl,
    &FindStructImpl,
    &FindStructPropertyImpl,
    &FindFunctionImpl,
    &AllocParamsImpl,
    &FreeParamsImpl,
    &CallFunctionImpl,
    &GetFunctionParamImpl,
    &GetPropertyOffsetImpl,
    &FindFunctionByClassImpl,
    &GetElementSizeImpl,
    &GetStructSizeImpl,
    &InitializeStructImpl,
    &DestroyStructImpl,
};
