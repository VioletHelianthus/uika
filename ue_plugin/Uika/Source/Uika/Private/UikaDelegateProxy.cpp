#include "UikaDelegateProxy.h"
#include "UikaApiTable.h"

#include "UikaModule.h"

// Static member initialization.
FName UUikaDelegateProxy::FakeFuncName(TEXT("RustFakeCallable"));

void UUikaDelegateProxy::RustFakeCallable()
{
    // Empty body. This UFUNCTION exists solely to register the FName
    // "RustFakeCallable" in UE's reflection system, so that
    // BindUFunction(Proxy, FakeFuncName) resolves correctly.
}

void UUikaDelegateProxy::ProcessEvent(UFunction* Function, void* Parms)
{
    // Normal path: if the function isn't our fake callable, delegate to Super.
    if (Function->GetFName() != FakeFuncName)
    {
        Super::ProcessEvent(Function, Parms);
        return;
    }

    // Delegate invocation path: forward to Rust.
    const FUikaRustCallbacks* Callbacks = GetUikaRustCallbacks();
    if (Callbacks && Callbacks->invoke_delegate_callback)
    {
        Callbacks->invoke_delegate_callback(CallbackId, static_cast<uint8*>(Parms));
    }
    else
    {
        UE_LOG(LogUika, Warning,
            TEXT("[Uika] DelegateProxy: Rust callbacks not available (CallbackId=%llu)"),
            CallbackId);
    }
}
