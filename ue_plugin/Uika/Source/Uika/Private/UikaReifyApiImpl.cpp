// UikaReifyApiImpl.cpp — FUikaReifyApi implementation + UObject delete listener.

#include "UikaApiTable.h"
#include "UUikaReifiedClass.h"
#include "UUikaReifiedFunction.h"
#include "UikaModule.h"
#include "Engine/Blueprint.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UnrealType.h"
#include "UObject/UObjectArray.h"
#include "UObject/UObjectIterator.h"

// Helper: convert UTF-8 byte slice to FName.
static FName ReifyUtf8ToFName(const uint8* Name, uint32 NameLen)
{
    const FString Str(NameLen, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Name)));
    return FName(*Str);
}

// Helper: convert UTF-8 byte slice to FString.
static FString ReifyUtf8ToFString(const uint8* Name, uint32 NameLen)
{
    return FString(NameLen, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Name)));
}

// ---------------------------------------------------------------------------
// Helper: Create an FProperty by type enum
// ---------------------------------------------------------------------------

static FProperty* CreatePropertyByType(
    FFieldVariant Owner,
    FName PropName,
    EUikaReifyPropType PropType,
    const FUikaReifyPropExtra* Extra)
{
    FProperty* Prop = nullptr;

    switch (PropType)
    {
    case EUikaReifyPropType::Bool:
    {
        FBoolProperty* BoolProp = new FBoolProperty(Owner, PropName, RF_Public);
        Prop = BoolProp;
        break;
    }
    case EUikaReifyPropType::Int8:
    {
        Prop = new FInt8Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Int16:
    {
        Prop = new FInt16Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Int32:
    {
        Prop = new FIntProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Int64:
    {
        Prop = new FInt64Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::UInt8:
    {
        Prop = new FByteProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::UInt16:
    {
        Prop = new FUInt16Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::UInt32:
    {
        Prop = new FUInt32Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::UInt64:
    {
        Prop = new FUInt64Property(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Float:
    {
        Prop = new FFloatProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Double:
    {
        Prop = new FDoubleProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::String:
    {
        Prop = new FStrProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Name:
    {
        Prop = new FNameProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Text:
    {
        Prop = new FTextProperty(Owner, PropName, RF_Public);
        break;
    }
    case EUikaReifyPropType::Object:
    {
        FObjectProperty* ObjProp = new FObjectProperty(Owner, PropName, RF_Public);
        if (Extra && Extra->class_handle.ptr)
        {
            ObjProp->PropertyClass = static_cast<UClass*>(Extra->class_handle.ptr);
        }
        else
        {
            ObjProp->PropertyClass = UObject::StaticClass();
        }
        Prop = ObjProp;
        break;
    }
    case EUikaReifyPropType::Class:
    {
        FClassProperty* ClsProp = new FClassProperty(Owner, PropName, RF_Public);
        if (Extra)
        {
            ClsProp->PropertyClass = Extra->class_handle.ptr
                ? static_cast<UClass*>(Extra->class_handle.ptr)
                : UObject::StaticClass();
            ClsProp->MetaClass = Extra->meta_class_handle.ptr
                ? static_cast<UClass*>(Extra->meta_class_handle.ptr)
                : UObject::StaticClass();
        }
        else
        {
            ClsProp->PropertyClass = UObject::StaticClass();
            ClsProp->MetaClass = UObject::StaticClass();
        }
        Prop = ClsProp;
        break;
    }
    case EUikaReifyPropType::Struct:
    {
        UScriptStruct* ScriptStruct = (Extra && Extra->struct_handle.ptr)
            ? static_cast<UScriptStruct*>(Extra->struct_handle.ptr)
            : nullptr;
        if (!ScriptStruct)
        {
            UE_LOG(LogUika, Error, TEXT("[Uika] CreatePropertyByType(Struct): null struct_handle"));
            return nullptr;
        }
        FStructProperty* StructProp = new FStructProperty(Owner, PropName, RF_Public);
        StructProp->Struct = ScriptStruct;
        Prop = StructProp;
        break;
    }
    case EUikaReifyPropType::Enum:
    {
        UEnum* EnumType = (Extra && Extra->enum_handle.ptr)
            ? static_cast<UEnum*>(Extra->enum_handle.ptr)
            : nullptr;
        if (!EnumType)
        {
            UE_LOG(LogUika, Error, TEXT("[Uika] CreatePropertyByType(Enum): null enum_handle"));
            return nullptr;
        }
        FEnumProperty* EnumProp = new FEnumProperty(Owner, PropName, RF_Public);
        EnumProp->SetEnum(EnumType);
        // Create the underlying numeric property
        FNumericProperty* UnderlyingProp = new FByteProperty(EnumProp, TEXT("UnderlyingType"), RF_Public);
        EnumProp->AddCppProperty(UnderlyingProp);
        Prop = EnumProp;
        break;
    }
    default:
        UE_LOG(LogUika, Error, TEXT("[Uika] CreatePropertyByType: unknown type %d"), static_cast<int>(PropType));
        return nullptr;
    }

    return Prop;
}

// ---------------------------------------------------------------------------
// API implementations
// ---------------------------------------------------------------------------

// Shared package pointer for all reified classes.
static UPackage* GUikaReifyPackage = nullptr;

static UPackage* GetOrCreateUikaPackage()
{
    if (!GUikaReifyPackage)
    {
        GUikaReifyPackage = CreatePackage(TEXT("/Script/Uika"));
        GUikaReifyPackage->SetPackageFlags(PKG_CompiledIn);
    }
    return GUikaReifyPackage;
}

static UikaUClassHandle CreateClassImpl(
    const uint8* Name, uint32 NameLen,
    UikaUClassHandle Parent,
    uint64 RustTypeId)
{
    UClass* ParentClass = static_cast<UClass*>(Parent.ptr);
    if (!ParentClass)
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] CreateClass: null parent class"));
        return UikaUClassHandle{ nullptr };
    }

    const FString ClassName = ReifyUtf8ToFString(Name, NameLen);

    // --- Hot reload path: if a class with this name already exists, reuse it ---
    UPackage* UikaPackage = GetOrCreateUikaPackage();
    UUikaReifiedClass* Existing = FindObject<UUikaReifiedClass>(
        UikaPackage, *ClassName);
    if (Existing)
    {
        // Update the Rust type ID (may have changed if Rust struct layout changed).
        Existing->RustTypeId = RustTypeId;

        UE_LOG(LogUika, Display,
            TEXT("[Uika] Hot reload: reusing existing class %s (type_id: %llu)"),
            *ClassName, RustTypeId);

        return UikaUClassHandle{ Existing };
    }

    // --- Normal path: create new class ---
    UUikaReifiedClass* NewClass = NewObject<UUikaReifiedClass>(
        UikaPackage,
        FName(*ClassName),
        RF_Public | RF_Standalone);

    NewClass->RustTypeId = RustTypeId;

    // Walk up to find the native (C++) superclass.
    UClass* NativeSuper = ParentClass;
    while (NativeSuper && !NativeSuper->HasAnyClassFlags(CLASS_Native))
    {
        NativeSuper = NativeSuper->GetSuperClass();
    }
    NewClass->NativeSuperClass = NativeSuper ? NativeSuper : ParentClass;

    // Set up class hierarchy.
    NewClass->SetSuperStruct(ParentClass);
    NewClass->ClassConstructor = &UUikaReifiedClass::UikaClassConstructor;

    // Propagate inheritable flags from parent (CLASS_HasInstancedReference, etc.).
    // Bind() propagates ClassCastFlags but NOT CLASS_Inherit flags.
    // Exclude config-related flags: dynamically-created classes don't have a
    // ClassConfigName and would crash in GetConfigName()/LoadConfig().
    constexpr EClassFlags ConfigRelatedFlags = EClassFlags(
        CLASS_Config | CLASS_DefaultConfig | CLASS_PerObjectConfig |
        CLASS_ConfigDoNotCheckDefaults | CLASS_GlobalUserConfig |
        CLASS_ProjectUserConfig | CLASS_PerPlatformConfig);
    NewClass->ClassFlags |= (ParentClass->ClassFlags & CLASS_Inherit & ~ConfigRelatedFlags) | CLASS_CompiledFromBlueprint;

    // Create a stub UBlueprint so that FBlueprintActionDatabase registers
    // our functions.  Without this, the action database sees our class as a
    // UBlueprintGeneratedClass with null ClassGeneratedBy and skips it.
    UBlueprint* StubBP = NewObject<UBlueprint>(
        UikaPackage, FName(*(ClassName + TEXT("_BP"))),
        RF_Public | RF_Standalone);
    StubBP->GeneratedClass = NewClass;
    StubBP->SkeletonGeneratedClass = NewClass;
    StubBP->ParentClass = ParentClass;
    StubBP->BlueprintType = BPTYPE_Normal;
    StubBP->Status = BS_UpToDate;
    StubBP->AddToRoot();
    NewClass->ClassGeneratedBy = StubBP;

#if WITH_EDITORONLY_DATA
    // Mark as "cooked" so GetGeneratedClassesHierarchy skips the
    // BS_Error check (our stub UBlueprint is always up-to-date).
    NewClass->bCooked = true;
#endif

    // Prevent garbage collection.
    NewClass->AddToRoot();

    UE_LOG(LogUika, Display, TEXT("[Uika] Created reified class: %s (parent: %s, type_id: %llu)"),
        *ClassName, *ParentClass->GetName(), RustTypeId);

    return UikaUClassHandle{ NewClass };
}

static UikaFPropertyHandle AddPropertyImpl(
    UikaUClassHandle Cls,
    const uint8* Name, uint32 NameLen,
    uint32 PropType, uint64 PropFlags,
    const FUikaReifyPropExtra* Extra)
{
    UClass* Class = static_cast<UClass*>(Cls.ptr);
    if (!Class)
    {
        return UikaFPropertyHandle{ nullptr };
    }

    const FName PropName = ReifyUtf8ToFName(Name, NameLen);

    // --- Hot reload path: if a property with this name already exists, reuse it ---
    for (FProperty* P = Class->PropertyLink; P; P = P->PropertyLinkNext)
    {
        if (P->GetOwnerClass() == Class && P->GetFName() == PropName)
        {
            UE_LOG(LogUika, Display,
                TEXT("[Uika] Hot reload: reusing existing property %s::%s"),
                *Class->GetName(), *PropName.ToString());
            return UikaFPropertyHandle{ P };
        }
    }

    // --- Normal path: create new property ---
    FProperty* Prop = CreatePropertyByType(
        FFieldVariant(Class),
        PropName,
        static_cast<EUikaReifyPropType>(PropType),
        Extra);

    if (!Prop)
    {
        return UikaFPropertyHandle{ nullptr };
    }

    Prop->PropertyFlags |= static_cast<EPropertyFlags>(PropFlags);
    Class->AddCppProperty(Prop);

    return UikaFPropertyHandle{ Prop };
}

static UikaUFunctionHandle AddFunctionImpl(
    UikaUClassHandle Cls,
    const uint8* Name, uint32 NameLen,
    uint64 CallbackId, uint32 FuncFlags)
{
    UClass* Class = static_cast<UClass*>(Cls.ptr);
    if (!Class)
    {
        return UikaUFunctionHandle{ nullptr };
    }

    const FString FuncName = ReifyUtf8ToFString(Name, NameLen);

    // --- Hot reload path: if this function already exists, just update the callback ID ---
    UFunction* ExistingFunc = Class->FindFunctionByName(FName(*FuncName));
    if (ExistingFunc)
    {
        if (UUikaReifiedFunction* Reified = Cast<UUikaReifiedFunction>(ExistingFunc))
        {
            Reified->CallbackId = CallbackId;
            UE_LOG(LogUika, Display,
                TEXT("[Uika] Hot reload: updated CallbackId for %s::%s (id: %llu)"),
                *Class->GetName(), *FuncName, CallbackId);
            return UikaUFunctionHandle{ ExistingFunc };
        }
    }

    // --- Normal path: create new function ---
    UUikaReifiedFunction* NewFunc = NewObject<UUikaReifiedFunction>(
        Class, FName(*FuncName), RF_Public | RF_MarkAsNative);

    NewFunc->CallbackId = CallbackId;
    NewFunc->FunctionFlags = static_cast<EFunctionFlags>(FuncFlags) | FUNC_Native;

    // Set the native function pointer to the thunk.
    NewFunc->SetNativeFunc(&UUikaReifiedFunction::execCallRustFunction);

    // Link into the class's Children list so TFieldIterator<UFunction> can
    // discover it (used by Blueprint action menu, StaticLink, etc.).
    NewFunc->Next = Class->Children;
    Class->Children = NewFunc;

    // Register the native function name for the VM.
    Class->AddNativeFunction(*FuncName, &UUikaReifiedFunction::execCallRustFunction);
    Class->AddFunctionToFunctionMap(NewFunc, NewFunc->GetFName());

    return UikaUFunctionHandle{ NewFunc };
}

static EUikaErrorCode AddFunctionParamImpl(
    UikaUFunctionHandle Func,
    const uint8* Name, uint32 NameLen,
    uint32 PropType, uint64 ParamFlags,
    const FUikaReifyPropExtra* Extra)
{
    UUikaReifiedFunction* Function = Cast<UUikaReifiedFunction>(static_cast<UFunction*>(Func.ptr));
    if (!Function)
    {
        return EUikaErrorCode::NullArgument;
    }

    const FName ParamName = ReifyUtf8ToFName(Name, NameLen);

    // --- Hot reload path: if a param with this name already exists, reuse it ---
    for (FField* Field = Function->ChildProperties; Field; Field = Field->Next)
    {
        if (FProperty* Existing = CastField<FProperty>(Field))
        {
            if (Existing->GetFName() == ParamName)
            {
                return EUikaErrorCode::Ok;
            }
        }
    }

    // --- Normal path: create new parameter property ---
    FProperty* Param = CreatePropertyByType(
        FFieldVariant(Function),
        ParamName,
        static_cast<EUikaReifyPropType>(PropType),
        Extra);

    if (!Param)
    {
        return EUikaErrorCode::InternalError;
    }

    // Set parameter flags (CPF_Parm must always be set for function parameters).
    Param->PropertyFlags |= static_cast<EPropertyFlags>(ParamFlags) | CPF_Parm;

    // Append to the END of ChildProperties instead of using AddCppProperty
    // (which prepends). This keeps parameters in declaration order, matching
    // UHT convention. The Blueprint compiler, bytecode VM, and our thunk all
    // iterate ChildProperties in linked-list order, so they must agree.
    if (Function->ChildProperties == nullptr)
    {
        Function->ChildProperties = Param;
    }
    else
    {
        FField* Last = Function->ChildProperties;
        while (Last->Next)
        {
            Last = Last->Next;
        }
        Last->Next = Param;
    }

    return EUikaErrorCode::Ok;
}

static EUikaErrorCode FinalizeClassImpl(UikaUClassHandle Cls)
{
    UUikaReifiedClass* Class = Cast<UUikaReifiedClass>(static_cast<UClass*>(Cls.ptr));
    if (!Class)
    {
        return EUikaErrorCode::NullArgument;
    }

    // Hot reload path: if already finalized (Bind/StaticLink done), skip.
    if (Class->HasAnyClassFlags(CLASS_Constructed))
    {
        UE_LOG(LogUika, Display,
            TEXT("[Uika] Hot reload: class %s already finalized, skipping"),
            *Class->GetName());
        return EUikaErrorCode::Ok;
    }

    // Finalize function parameter layouts: Bind → StaticLink each function.
    for (TFieldIterator<UFunction> FuncIt(Class, EFieldIteratorFlags::ExcludeSuper); FuncIt; ++FuncIt)
    {
        UFunction* Func = *FuncIt;
        Func->Bind();
        Func->StaticLink(true);
    }

    // Finalize the class itself.
    Class->Bind();
    Class->StaticLink(true);

    // Build the GC reference token stream so the garbage collector can
    // properly trace UObject* references within instances of this class.
    Class->AssembleReferenceTokenStream(true);

    // Force CDO creation and run BPGC post-load initialization
    // (builds CustomPropertyListForPostConstruction, etc.).
    UObject* CDO = Class->GetDefaultObject(true);
    Class->PostLoadDefaultObject(CDO);

    UE_LOG(LogUika, Display, TEXT("[Uika] Finalized reified class: %s (size: %d, super_size: %d)"),
        *Class->GetName(), Class->GetPropertiesSize(),
        Class->GetSuperClass() ? Class->GetSuperClass()->GetPropertiesSize() : 0);

    // Validate property chain integrity.
    int32 PropCount = 0;
    for (FProperty* P = Class->PropertyLink; P; P = P->PropertyLinkNext)
    {
        if (P->GetOwnerClass() == Class)
        {
            UE_LOG(LogUika, Display, TEXT("[Uika]   Property: %s offset=%d size=%d"),
                *P->GetName(), P->GetOffset_ForInternal(), P->GetSize());
        }
        PropCount++;
        if (PropCount > 10000)
        {
            UE_LOG(LogUika, Error, TEXT("[Uika] PropertyLink chain appears corrupt (>10000 entries)"));
            break;
        }
    }
    UE_LOG(LogUika, Display, TEXT("[Uika]   Total properties in chain: %d"), PropCount);

    return EUikaErrorCode::Ok;
}

static UikaUObjectHandle GetCdoImpl(UikaUClassHandle Cls)
{
    UClass* Class = static_cast<UClass*>(Cls.ptr);
    if (!Class)
    {
        return UikaUObjectHandle{ nullptr };
    }
    return UikaUObjectHandle{ Class->GetDefaultObject() };
}

// ---------------------------------------------------------------------------
// Default subobject registration
// ---------------------------------------------------------------------------

static EUikaErrorCode AddDefaultSubobjectImpl(
    UikaUClassHandle Cls,
    const uint8* Name, uint32 NameLen,
    UikaUClassHandle CompClass,
    uint32 Flags,
    const uint8* AttachParent, uint32 AttachLen)
{
    UUikaReifiedClass* RC = Cast<UUikaReifiedClass>(static_cast<UClass*>(Cls.ptr));
    if (!RC) return EUikaErrorCode::InvalidCast;

    UClass* CompUClass = static_cast<UClass*>(CompClass.ptr);
    if (!CompUClass) return EUikaErrorCode::PropertyNotFound;

    FUikaComponentDef Def;
    Def.SubobjectName = FName(ReifyUtf8ToFString(Name, NameLen));
    Def.ComponentClass = CompUClass;
    Def.bIsRoot = (Flags & 1) != 0;
    Def.bIsTransient = (Flags & 2) != 0;
    Def.AttachParentName = AttachLen > 0
        ? FName(ReifyUtf8ToFString(AttachParent, AttachLen))
        : NAME_None;

    // Hot reload: avoid duplicate defs
    RC->ComponentDefs.RemoveAll([&](const FUikaComponentDef& D) {
        return D.SubobjectName == Def.SubobjectName;
    });
    RC->ComponentDefs.Add(MoveTemp(Def));

    UE_LOG(LogUika, Display, TEXT("[Uika] Registered default subobject '%s' (class: %s) on %s"),
        *Def.SubobjectName.ToString(), *CompUClass->GetName(), *RC->GetName());

    return EUikaErrorCode::Ok;
}

static UikaUObjectHandle FindDefaultSubobjectImpl(
    UikaUObjectHandle Owner,
    const uint8* Name, uint32 NameLen)
{
    UObject* Obj = static_cast<UObject*>(Owner.ptr);
    if (!Obj) return UikaUObjectHandle{ nullptr };

    FName SubName(ReifyUtf8ToFString(Name, NameLen));
    UObject* Sub = Obj->GetDefaultSubobjectByName(SubName);
    return UikaUObjectHandle{ Sub };
}

// ---------------------------------------------------------------------------
// FUikaDeleteListener — Notifies Rust when a reified-class instance is GC'd
// ---------------------------------------------------------------------------

class FUikaDeleteListener : public FUObjectArray::FUObjectDeleteListener
{
public:
    virtual void NotifyUObjectDeleted(const UObjectBase* Object, int32 Index) override
    {
        // Only handle objects whose class is a reified class.
        const UClass* ObjClass = Object->GetClass();
        const UUikaReifiedClass* ReifiedClass = Cast<UUikaReifiedClass>(ObjClass);
        if (!ReifiedClass)
        {
            return;
        }

        const FUikaRustCallbacks* Callbacks = GetUikaRustCallbacks();
        if (Callbacks && Callbacks->drop_rust_instance)
        {
            Callbacks->drop_rust_instance(
                UikaUObjectHandle{ const_cast<UObjectBase*>(Object) },
                ReifiedClass->RustTypeId,
                nullptr);
        }
    }

    virtual void OnUObjectArrayShutdown() override
    {
        GUObjectArray.RemoveUObjectDeleteListener(this);
    }
};

static FUikaDeleteListener GDeleteListener;

void UikaReifyRegisterDeleteListener()
{
    GUObjectArray.AddUObjectDeleteListener(&GDeleteListener);
}

void UikaReifyUnregisterDeleteListener()
{
    GUObjectArray.RemoveUObjectDeleteListener(&GDeleteListener);
}

// ---------------------------------------------------------------------------
// Hot reload helpers (called from UikaModule.cpp)
// ---------------------------------------------------------------------------

void UikaReifyForEachReifiedInstance(
    TFunctionRef<void(UObject*, UUikaReifiedClass*)> Callback)
{
    for (FThreadSafeObjectIterator It; It; ++It)
    {
        UObject* Obj = static_cast<UObject*>(*It);
        // Skip CDOs — they don't have meaningful Rust instance data.
        if (Obj->HasAnyFlags(RF_ClassDefaultObject))
        {
            continue;
        }
        UUikaReifiedClass* ReifiedClass = Cast<UUikaReifiedClass>(Obj->GetClass());
        if (ReifiedClass)
        {
            Callback(Obj, ReifiedClass);
        }
    }
}

// ---------------------------------------------------------------------------
// Export the API table
// ---------------------------------------------------------------------------

FUikaReifyApi GReifyApi = {
    &CreateClassImpl,
    &AddPropertyImpl,
    &AddFunctionImpl,
    &AddFunctionParamImpl,
    &FinalizeClassImpl,
    &GetCdoImpl,
    &AddDefaultSubobjectImpl,
    &FindDefaultSubobjectImpl,
};
