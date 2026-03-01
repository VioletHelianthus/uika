#pragma once

#include "Modules/ModuleManager.h"

DECLARE_LOG_CATEGORY_EXTERN(LogUika, Log, All);

class FUikaModule : public IModuleInterface
{
public:
    virtual void StartupModule() override;
    virtual void ShutdownModule() override;

    /** Unload the current Rust DLL, copy the new one, and reload. */
    void ReloadRustDll();

    /** Reload only the WASM module without DLL swap. */
    void ReloadWasm();

    /** Static entry point for the Uika.Reload console command. */
    static void StaticReload();

    /** Static entry point for the Uika.ReloadWasm console command. */
    static void StaticReloadWasm();

private:
    /** Drop Rust instance data for all reified objects. */
    void TeardownReifiedInstances();

    /** Reconstruct Rust instance data for all reified objects. */
    void ReconstructReifiedInstances();
    /** Unload the Rust DLL (teardown phase of reload, and used by ShutdownModule). */
    void UnloadRustDll();

    /** Load a Rust DLL from the given path and initialize it. */
    bool LoadRustDll(const FString& LoadPath);

    void* DllHandle = nullptr;
    const struct FUikaRustCallbacks* RustCallbacks = nullptr;

    /** Canonical path to uika.dll (cargo build output). */
    FString DllSourcePath;

    /** Path of the currently loaded DLL (may be a hot-copy). */
    FString CurrentLoadedDllPath;

    /** Incrementing counter for copy-on-reload filenames. */
    int32 ReloadCount = 0;
};
