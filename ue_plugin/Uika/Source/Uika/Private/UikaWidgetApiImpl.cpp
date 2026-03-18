// UikaWidgetApiImpl.cpp — FUikaWidgetApi implementation.
// Provides CreateWidget (C++ template, not in reflection), WidgetTree, and RootWidget access.

#include "UikaApiTable.h"
#include "Blueprint/UserWidget.h"
#include "Blueprint/WidgetTree.h"
#include "Components/Widget.h"
#include "GameFramework/PlayerController.h"
#include "Engine/GameInstance.h"
#include "Engine/World.h"

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

static UikaUObjectHandle CreateWidgetImpl(
    UikaUObjectHandle OwningHandle,
    UikaUClassHandle WidgetClassHandle)
{
    UClass* WidgetClass = static_cast<UClass*>(WidgetClassHandle.ptr);
    UObject* Owning = static_cast<UObject*>(OwningHandle.ptr);
    if (!WidgetClass || !Owning)
    {
        return UikaUObjectHandle{ nullptr };
    }

    // Try casting to supported owning types in order of likelihood.
    if (APlayerController* PC = Cast<APlayerController>(Owning))
    {
        UUserWidget* Widget = CreateWidget<UUserWidget>(PC, WidgetClass);
        return UikaUObjectHandle{ Widget };
    }
    if (UWorld* World = Cast<UWorld>(Owning))
    {
        UUserWidget* Widget = CreateWidget<UUserWidget>(World, WidgetClass);
        return UikaUObjectHandle{ Widget };
    }
    if (UGameInstance* GI = Cast<UGameInstance>(Owning))
    {
        UUserWidget* Widget = CreateWidget<UUserWidget>(GI, WidgetClass);
        return UikaUObjectHandle{ Widget };
    }

    // Fallback: try GetWorld() on the owning object.
    UWorld* World = Owning->GetWorld();
    if (World)
    {
        UUserWidget* Widget = CreateWidget<UUserWidget>(World, WidgetClass);
        return UikaUObjectHandle{ Widget };
    }

    return UikaUObjectHandle{ nullptr };
}

static EUikaErrorCode SetRootWidgetImpl(
    UikaUObjectHandle UserWidgetHandle,
    UikaUObjectHandle RootWidgetHandle)
{
    UUserWidget* UserWidget = Cast<UUserWidget>(static_cast<UObject*>(UserWidgetHandle.ptr));
    UWidget* RootWidget = Cast<UWidget>(static_cast<UObject*>(RootWidgetHandle.ptr));
    if (!UserWidget) return EUikaErrorCode::NullArgument;
    if (!RootWidget) return EUikaErrorCode::NullArgument;

    UWidgetTree* Tree = UserWidget->WidgetTree;
    if (!Tree) return EUikaErrorCode::InvalidOperation;

    Tree->RootWidget = RootWidget;
    return EUikaErrorCode::Ok;
}

static UikaUObjectHandle GetWidgetTreeImpl(UikaUObjectHandle UserWidgetHandle)
{
    UUserWidget* UserWidget = Cast<UUserWidget>(static_cast<UObject*>(UserWidgetHandle.ptr));
    if (!UserWidget) return UikaUObjectHandle{ nullptr };
    return UikaUObjectHandle{ UserWidget->WidgetTree };
}

// ---------------------------------------------------------------------------
// Static instance
// ---------------------------------------------------------------------------

FUikaWidgetApi GWidgetApi = {
    &CreateWidgetImpl,
    &SetRootWidgetImpl,
    &GetWidgetTreeImpl,
};
