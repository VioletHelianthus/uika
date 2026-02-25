#pragma once

#include "CoreMinimal.h"
#include "UObject/Object.h"
#include "UikaDelegateProxy.generated.h"

struct FUikaRustCallbacks;

// Proxy UObject that bridges UE delegates to Rust closures.
// Uses the FakeFuncName mechanism: the proxy is bound to a delegate via
// BindUFunction(Proxy, FakeFuncName). When the delegate fires, UE calls
// ProcessEvent on this proxy, which forwards to the Rust callback registry.
UCLASS()
class UUikaDelegateProxy : public UObject
{
    GENERATED_BODY()

public:
    // Rust-side callback ID (indexes into the delegate registry).
    uint64 CallbackId = 0;

    // The signature UFunction of the delegate this proxy is bound to.
    // Used by UE to validate parameter compatibility.
    UFunction* Signature = nullptr;

    // Weak reference to the object that owns this delegate binding.
    UPROPERTY()
    TObjectPtr<UObject> OwnerObject;

    // The FName used for BindUFunction — must match a UFUNCTION on this class.
    static FName FakeFuncName;

    // Empty UFUNCTION that registers FakeFuncName in the UE reflection system.
    UFUNCTION()
    void RustFakeCallable();

    // Override ProcessEvent to intercept delegate invocations.
    virtual void ProcessEvent(UFunction* Function, void* Parms) override;
};

// Module-level accessor for the Rust callbacks pointer.
const FUikaRustCallbacks* GetUikaRustCallbacks();
