#include "UUikaReifiedClass.h"
#include "UikaApiTable.h"
#include "UikaModule.h"
#include "Components/SceneComponent.h"
#include "GameFramework/Actor.h"

UClass* UUikaReifiedClass::GetAuthoritativeClass()
{
    // Reified classes have no UBlueprint asset, so this class IS the
    // authoritative class.  The base implementation would crash on the
    // null ClassGeneratedBy pointer.
    return this;
}

void UUikaReifiedClass::UikaClassConstructor(const FObjectInitializer& ObjectInitializer)
{
    // 1. Find the UUikaReifiedClass in the hierarchy. The immediate class may be
    //    a Blueprint child (e.g. SKEL_new_MacroTestActor_C), so walk up.
    UUikaReifiedClass* ReifiedClass = nullptr;
    for (UClass* Cls = ObjectInitializer.GetClass(); Cls; Cls = Cls->GetSuperClass())
    {
        ReifiedClass = Cast<UUikaReifiedClass>(Cls);
        if (ReifiedClass) break;
    }
    if (!ReifiedClass)
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] UikaClassConstructor called on non-reified class!"));
        return;
    }

    // 2. Call the native super's constructor to initialize UE-side state.
    UClass* NativeSuper = ReifiedClass->NativeSuperClass;
    if (NativeSuper && NativeSuper->ClassConstructor)
    {
        NativeSuper->ClassConstructor(ObjectInitializer);
    }

    // 3. Create default subobjects from Rust-registered definitions.
    UObject* Obj = ObjectInitializer.GetObj();
    if (ReifiedClass->ComponentDefs.Num() > 0)
    {
        TMap<FName, USceneComponent*> CreatedComponents;

        for (const FUikaComponentDef& Def : ReifiedClass->ComponentDefs)
        {
            UObject* Sub = ObjectInitializer.CreateDefaultSubobject(
                Obj, Def.SubobjectName,
                Def.ComponentClass, Def.ComponentClass,
                /*bIsRequired=*/true, Def.bIsTransient);

            if (!Sub) continue;

            USceneComponent* SceneComp = Cast<USceneComponent>(Sub);
            if (SceneComp)
            {
                CreatedComponents.Add(Def.SubobjectName, SceneComp);
            }

            if (Def.bIsRoot)
            {
                if (AActor* Actor = Cast<AActor>(Obj))
                {
                    if (SceneComp) Actor->SetRootComponent(SceneComp);
                }
            }
            else if (Def.AttachParentName != NAME_None)
            {
                if (SceneComp)
                {
                    if (USceneComponent** Parent = CreatedComponents.Find(Def.AttachParentName))
                    {
                        SceneComp->SetupAttachment(*Parent);
                    }
                }
            }
        }
    }

    // 4. Notify Rust to construct its instance data.
    const FUikaRustCallbacks* Callbacks = GetUikaRustCallbacks();
    if (Callbacks && Callbacks->construct_rust_instance)
    {
        bool bIsCDO = Obj->HasAnyFlags(RF_ClassDefaultObject);
        Callbacks->construct_rust_instance(
            UikaUObjectHandle{ Obj },
            ReifiedClass->RustTypeId,
            bIsCDO);
    }
}
