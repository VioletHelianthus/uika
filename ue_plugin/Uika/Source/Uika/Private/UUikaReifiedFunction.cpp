#include "UUikaReifiedFunction.h"
#include "UUikaReifiedClass.h"
#include "UikaApiTable.h"
#include "UikaModule.h"

DEFINE_FUNCTION(UUikaReifiedFunction::execCallRustFunction)
{
    // ---------------------------------------------------------------
    // Step 1: Find the UUikaReifiedFunction being called.
    //
    // When called from bytecode (EX_FinalFunction / EX_LocalFinalFunction),
    // Stack.Node is the CALLER's function (e.g., the Ubergraph), not ours.
    // UFunction::Invoke() sets Stack.CurrentNativeFunction before calling
    // our Func pointer, so we use that to identify ourselves.
    // ---------------------------------------------------------------

    UUikaReifiedFunction* ReifiedFunc = nullptr;

    if (Stack.CurrentNativeFunction)
    {
        ReifiedFunc = Cast<UUikaReifiedFunction>(Stack.CurrentNativeFunction);
    }
    if (!ReifiedFunc)
    {
        ReifiedFunc = Cast<UUikaReifiedFunction>(Stack.Node);
    }
    if (!ReifiedFunc)
    {
        UFunction* StartFunc = Stack.CurrentNativeFunction
            ? Stack.CurrentNativeFunction : Stack.Node;
        for (UFunction* F = StartFunc ? StartFunc->GetSuperFunction() : nullptr;
             F; F = F->GetSuperFunction())
        {
            ReifiedFunc = Cast<UUikaReifiedFunction>(F);
            if (ReifiedFunc) break;
        }
    }
    if (!ReifiedFunc && P_THIS)
    {
        UFunction* LookupFunc = Stack.CurrentNativeFunction
            ? Stack.CurrentNativeFunction : Stack.Node;
        if (LookupFunc)
        {
            FName FuncName = LookupFunc->GetFName();
            for (UClass* Cls = P_THIS->GetClass(); Cls; Cls = Cls->GetSuperClass())
            {
                if (UUikaReifiedClass* RC = Cast<UUikaReifiedClass>(Cls))
                {
                    if (UFunction* Found = RC->FindFunctionByName(
                            FuncName, EIncludeSuperFlag::ExcludeSuper))
                    {
                        ReifiedFunc = Cast<UUikaReifiedFunction>(Found);
                        if (ReifiedFunc) break;
                    }
                }
            }
        }
    }

    if (!ReifiedFunc)
    {
        UE_LOG(LogUika, Error,
            TEXT("[Uika] execCallRustFunction: cannot find UUikaReifiedFunction")
            TEXT(" (Node='%s', CurrentNative='%s')"),
            Stack.Node ? *Stack.Node->GetName() : TEXT("(null)"),
            Stack.CurrentNativeFunction
                ? *Stack.CurrentNativeFunction->GetName() : TEXT("(null)"));
        P_FINISH;
        return;
    }

    // ---------------------------------------------------------------
    // Step 2: Read parameters.
    //
    // ProcessEvent path (Stack.Node == ReifiedFunc):
    //   Locals already contain our params.
    //
    // Bytecode path (Stack.Node != ReifiedFunc):
    //   Read each input param from the bytecode using Stack.Step(),
    //   then P_FINISH to skip past EX_EndFunctionParms.
    //
    // NOTE: We walk ChildProperties directly instead of TFieldIterator
    // because TFieldIterator uses the PropertyLink chain which may not
    // be populated for dynamically-created functions.
    // ---------------------------------------------------------------

    const bool bFromProcessEvent = (Stack.Node == ReifiedFunc);
    uint8* ParamsPtr = nullptr;

    if (bFromProcessEvent)
    {
        P_FINISH;
        ParamsPtr = Stack.Locals;
    }
    else
    {
        const int32 PropsSize = ReifiedFunc->PropertiesSize;
        if (PropsSize > 0)
        {
            ParamsPtr = (uint8*)FMemory_Alloca(PropsSize);
            FMemory::Memzero(ParamsPtr, PropsSize);

            // Walk ChildProperties directly to read input params from bytecode.
            for (FField* Field = ReifiedFunc->ChildProperties; Field; Field = Field->Next)
            {
                FProperty* Prop = CastField<FProperty>(Field);
                if (!Prop) continue;
                // Skip return value — only read input params from bytecode.
                if (Prop->HasAnyPropertyFlags(CPF_ReturnParm)) continue;
                if (!Prop->HasAnyPropertyFlags(CPF_Parm)) continue;

                Stack.Step(Stack.Object, ParamsPtr + Prop->GetOffset_ForUFunction());
            }
        }
        P_FINISH;
    }

    // ---------------------------------------------------------------
    // Step 3: Forward to Rust via the callback table.
    // ---------------------------------------------------------------

    const FUikaRustCallbacks* Callbacks = GetUikaRustCallbacks();
    if (Callbacks && Callbacks->invoke_rust_function)
    {
        Callbacks->invoke_rust_function(
            ReifiedFunc->CallbackId,
            UikaUObjectHandle{ P_THIS },
            ParamsPtr);
    }

    // Copy return value to RESULT_PARAM.
    if (RESULT_PARAM && ParamsPtr)
    {
        if (FProperty* RetProp = ReifiedFunc->GetReturnProperty())
        {
            RetProp->CopyCompleteValue(
                RESULT_PARAM,
                ParamsPtr + RetProp->GetOffset_ForUFunction());
        }
    }

    // Destroy temporary parameter values for the bytecode path.
    if (!bFromProcessEvent && ParamsPtr)
    {
        for (FField* Field = ReifiedFunc->ChildProperties; Field; Field = Field->Next)
        {
            FProperty* Prop = CastField<FProperty>(Field);
            if (Prop && Prop->HasAnyPropertyFlags(CPF_Parm))
            {
                Prop->DestroyValue_InContainer(ParamsPtr);
            }
        }
    }
}
