// UikaWorldApiImpl.cpp — FUikaWorldApi implementation.

#include "UikaApiTable.h"
#include "UObject/UObjectGlobals.h"
#include "Engine/World.h"
#include "GameFramework/Actor.h"
#include "GameFramework/Pawn.h"
#include "EngineUtils.h"

// Helper: convert UTF-8 byte slice to FString.
static FString Utf8ToFStr(const uint8* Buf, uint32 Len)
{
    return FString(Len, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Buf)));
}

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

static UikaUObjectHandle SpawnActorImpl(
    UikaUObjectHandle WorldHandle,
    UikaUClassHandle ClsHandle,
    const uint8* TransformBuf,
    uint32 TransformSize,
    UikaUObjectHandle OwnerHandle)
{
    UWorld* World = Cast<UWorld>(static_cast<UObject*>(WorldHandle.ptr));
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    if (!World || !Class)
    {
        return UikaUObjectHandle{ nullptr };
    }

    // Copy transform from Rust buffer. The buffer comes from UScriptStruct::GetStructureSize()
    // which may be smaller than sizeof(FTransform) due to C++ SIMD alignment padding.
    // Copy what we have and leave the rest as identity.
    FTransform SpawnTransform = FTransform::Identity;
    if (TransformBuf && TransformSize > 0)
    {
        const uint32 CopySize = FMath::Min(TransformSize, static_cast<uint32>(sizeof(FTransform)));
        FMemory::Memcpy(&SpawnTransform, TransformBuf, CopySize);
    }

    FActorSpawnParameters Params;
    AActor* Owner = Cast<AActor>(static_cast<UObject*>(OwnerHandle.ptr));
    if (Owner)
    {
        Params.Owner = Owner;
    }

    AActor* Spawned = World->SpawnActor(Class, &SpawnTransform, Params);
    return UikaUObjectHandle{ Spawned };
}

static EUikaErrorCode GetAllActorsOfClassImpl(
    UikaUObjectHandle WorldHandle,
    UikaUClassHandle ClsHandle,
    UikaUObjectHandle* OutBuf,
    uint32 BufCapacity,
    uint32* OutCount)
{
    UWorld* World = Cast<UWorld>(static_cast<UObject*>(WorldHandle.ptr));
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    if (!World || !Class)
    {
        if (OutCount) *OutCount = 0;
        return EUikaErrorCode::NullArgument;
    }

    uint32 Count = 0;
    for (TActorIterator<AActor> It(World, Class); It; ++It)
    {
        if (OutBuf && Count < BufCapacity)
        {
            OutBuf[Count] = UikaUObjectHandle{ *It };
        }
        ++Count;
    }

    if (OutCount) *OutCount = Count;
    return EUikaErrorCode::Ok;
}

static UikaUObjectHandle FindObjectImpl(
    UikaUClassHandle ClsHandle,
    const uint8* PathUtf8,
    uint32 PathLen)
{
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    const FString Path = Utf8ToFStr(PathUtf8, PathLen);

    UObject* Found = StaticFindObject(Class, nullptr, *Path);
    return UikaUObjectHandle{ Found };
}

static UikaUObjectHandle LoadObjectImpl(
    UikaUClassHandle ClsHandle,
    const uint8* PathUtf8,
    uint32 PathLen)
{
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    if (!Class) Class = UObject::StaticClass();
    const FString Path = Utf8ToFStr(PathUtf8, PathLen);

    UObject* Loaded = StaticLoadObject(Class, nullptr, *Path);
    return UikaUObjectHandle{ Loaded };
}

static UikaUObjectHandle GetWorldImpl(UikaUObjectHandle ActorHandle)
{
    AActor* Actor = Cast<AActor>(static_cast<UObject*>(ActorHandle.ptr));
    if (!Actor) return UikaUObjectHandle{ nullptr };
    UWorld* World = Actor->GetWorld();
    return UikaUObjectHandle{ World };
}

static UikaUObjectHandle NewObjectImpl(UikaUObjectHandle OuterHandle, UikaUClassHandle ClsHandle)
{
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    if (!Class) return UikaUObjectHandle{ nullptr };
    UObject* Outer = static_cast<UObject*>(OuterHandle.ptr);
    if (!Outer) Outer = GetTransientPackage();
    UObject* Obj = NewObject<UObject>(Outer, Class);
    return UikaUObjectHandle{ Obj };
}

static UikaUObjectHandle SpawnActorDeferredImpl(
    UikaUObjectHandle WorldHandle,
    UikaUClassHandle ClsHandle,
    const uint8* TransformBuf,
    uint32 TransformSize,
    UikaUObjectHandle OwnerHandle,
    UikaUObjectHandle InstigatorHandle,
    uint8 CollisionMethod)
{
    UWorld* World = Cast<UWorld>(static_cast<UObject*>(WorldHandle.ptr));
    UClass* Class = static_cast<UClass*>(ClsHandle.ptr);
    if (!World || !Class)
    {
        return UikaUObjectHandle{ nullptr };
    }

    FTransform SpawnTransform = FTransform::Identity;
    if (TransformBuf && TransformSize > 0)
    {
        const uint32 CopySize = FMath::Min(TransformSize, static_cast<uint32>(sizeof(FTransform)));
        FMemory::Memcpy(&SpawnTransform, TransformBuf, CopySize);
    }

    FActorSpawnParameters Params;
    Params.bDeferConstruction = true;
    Params.SpawnCollisionHandlingOverride =
        static_cast<ESpawnActorCollisionHandlingMethod>(CollisionMethod);

    AActor* Owner = Cast<AActor>(static_cast<UObject*>(OwnerHandle.ptr));
    if (Owner)
    {
        Params.Owner = Owner;
    }

    APawn* Instigator = Cast<APawn>(static_cast<UObject*>(InstigatorHandle.ptr));
    if (Instigator)
    {
        Params.Instigator = Instigator;
    }

    AActor* Spawned = World->SpawnActor(Class, &SpawnTransform, Params);
    return UikaUObjectHandle{ Spawned };
}

static EUikaErrorCode FinishSpawningImpl(
    UikaUObjectHandle ActorHandle,
    const uint8* TransformBuf,
    uint32 TransformSize)
{
    AActor* Actor = Cast<AActor>(static_cast<UObject*>(ActorHandle.ptr));
    if (!Actor) return EUikaErrorCode::NullArgument;

    FTransform SpawnTransform = FTransform::Identity;
    if (TransformBuf && TransformSize > 0)
    {
        const uint32 CopySize = FMath::Min(TransformSize, static_cast<uint32>(sizeof(FTransform)));
        FMemory::Memcpy(&SpawnTransform, TransformBuf, CopySize);
    }

    Actor->FinishSpawning(SpawnTransform);
    return EUikaErrorCode::Ok;
}

// ---------------------------------------------------------------------------
// Static instance
// ---------------------------------------------------------------------------

FUikaWorldApi GWorldApi = {
    &SpawnActorImpl,
    &GetAllActorsOfClassImpl,
    &FindObjectImpl,
    &LoadObjectImpl,
    &GetWorldImpl,
    &NewObjectImpl,
    &SpawnActorDeferredImpl,
    &FinishSpawningImpl,
};
