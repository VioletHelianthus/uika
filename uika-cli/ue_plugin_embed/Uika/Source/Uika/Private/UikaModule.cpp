#include "UikaModule.h"
#include "UikaApiTable.h"
#include "UUikaReifiedClass.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformFileManager.h"
#include "HAL/FileManager.h"
#include "Misc/Paths.h"

DEFINE_LOG_CATEGORY(LogUika);

// External API sub-table instances (defined in their respective *Impl.cpp files)
extern FUikaCoreApi       GCoreApi;
extern FUikaReflectionApi GReflectionApi;
extern FUikaPropertyApi   GPropertyApi;
extern FUikaContainerApi  GContainerApi;
extern FUikaDelegateApi   GDelegateApi;
extern FUikaLifecycleApi  GLifecycleApi;
extern FUikaReifyApi      GReifyApi;
extern FUikaWorldApi      GWorldApi;

// Reify helpers (defined in UikaReifyApiImpl.cpp)
extern void UikaReifyRegisterDeleteListener();
extern void UikaReifyUnregisterDeleteListener();

// Pinned lifecycle helpers (defined in UikaLifecycleApiImpl.cpp)
extern void UikaPinnedUnregisterDeleteListener();
extern void UikaReifyForEachReifiedInstance(
    TFunctionRef<void(UObject*, UUikaReifiedClass*)> Callback);

// Module-level storage for Rust callbacks (set during StartupModule, read by UUikaDelegateProxy).
static const FUikaRustCallbacks* GRustCallbacks = nullptr;

const FUikaRustCallbacks* GetUikaRustCallbacks()
{
    return GRustCallbacks;
}

// Forward declarations for generated func_table code
extern void UikaFillFuncTable();
extern void** UikaGetFuncTable();
extern uint32_t UikaGetFuncCount();

#define LOCTEXT_NAMESPACE "FUikaModule"

// ---------------------------------------------------------------------------
// Logging bridge (the one API implemented in Phase 1 for end-to-end testing)
// ---------------------------------------------------------------------------

static void UikaLogImpl(uint8 Level, const uint8* Msg, uint32 MsgLen)
{
    const FString MsgStr(MsgLen, UTF8_TO_TCHAR(reinterpret_cast<const char*>(Msg)));
    switch (Level)
    {
    case 0:  UE_LOG(LogUika, Display, TEXT("%s"), *MsgStr); break;
    case 1:  UE_LOG(LogUika, Warning, TEXT("%s"), *MsgStr); break;
    default: UE_LOG(LogUika, Error,   TEXT("%s"), *MsgStr); break;
    }
}

static FUikaLoggingApi GLoggingApi = { &UikaLogImpl };

// ---------------------------------------------------------------------------
// API table instance
// ---------------------------------------------------------------------------

static FUikaApiTable GApiTable;

static void FillApiTable()
{
    FMemory::Memzero(GApiTable);
    GApiTable.version = 1;

    // Implemented sub-tables
    GApiTable.logging    = &GLoggingApi;
    GApiTable.core       = &GCoreApi;
    GApiTable.property   = &GPropertyApi;
    GApiTable.reflection = &GReflectionApi;
    GApiTable.memory       = nullptr;
    GApiTable.container    = &GContainerApi;
    GApiTable.delegate     = &GDelegateApi;
    GApiTable.lifecycle    = &GLifecycleApi;
    GApiTable.reify        = &GReifyApi;
    GApiTable.world        = &GWorldApi;

    // Fill generated func_table (Phase 6)
    UikaFillFuncTable();
    GApiTable.func_table = reinterpret_cast<const void* const*>(UikaGetFuncTable());
    GApiTable.func_count = static_cast<uint32>(UikaGetFuncCount());
}

// ---------------------------------------------------------------------------
// Console command
// ---------------------------------------------------------------------------

static FAutoConsoleCommand CmdReload(
    TEXT("Uika.Reload"),
    TEXT("Hot-reload the Rust DLL (unload → copy → load)."),
    FConsoleCommandDelegate::CreateStatic(&FUikaModule::StaticReload));

void FUikaModule::StaticReload()
{
    FUikaModule& Module = FModuleManager::GetModuleChecked<FUikaModule>(TEXT("Uika"));
    Module.ReloadRustDll();
}

// ---------------------------------------------------------------------------
// Module lifecycle
// ---------------------------------------------------------------------------

void FUikaModule::StartupModule()
{
    // 1. Fill the API table
    FillApiTable();

    // 2. Locate the Rust DLL
    const FString PluginDir = FPaths::Combine(
        FPaths::ProjectPluginsDir(), TEXT("Uika"));
    DllSourcePath = FPaths::Combine(
        PluginDir, TEXT("Binaries"),
        FPlatformProcess::GetBinariesSubdirectory(),
        TEXT("uika.dll"));

    if (!FPaths::FileExists(DllSourcePath))
    {
        UE_LOG(LogUika, Warning,
            TEXT("[Uika] Rust DLL not found at %s — Rust side will not be loaded."),
            *DllSourcePath);
        return;
    }

    // 3. Copy-on-load: never lock the source DLL so that build.py / cargo
    //    can always overwrite it, and hot reload always reads the latest.
    ReloadCount++;
    const FString InitialCopyPath = FPaths::Combine(
        FPaths::GetPath(DllSourcePath),
        FString::Printf(TEXT("uika_hot_%d.dll"), ReloadCount));

    uint32 CopyResult = IFileManager::Get().Copy(*InitialCopyPath, *DllSourcePath);
    if (CopyResult != 0)
    {
        UE_LOG(LogUika, Error,
            TEXT("[Uika] Failed to copy DLL %s → %s (error %u). Falling back to direct load."),
            *DllSourcePath, *InitialCopyPath, CopyResult);
        // Fallback: load directly (will lock the source, but at least it works)
        if (!LoadRustDll(DllSourcePath))
        {
            return;
        }
    }
    else if (!LoadRustDll(InitialCopyPath))
    {
        return;
    }

    UE_LOG(LogUika, Display, TEXT("[Uika] Rust DLL loaded and initialized successfully."));
}

void FUikaModule::ShutdownModule()
{
    UnloadRustDll();

    // Clean up the hot-copy DLL (now unlocked).
    if (!CurrentLoadedDllPath.IsEmpty() && CurrentLoadedDllPath != DllSourcePath)
    {
        IFileManager::Get().Delete(*CurrentLoadedDllPath, false, true, true);
    }
}

// ---------------------------------------------------------------------------
// DLL load / unload helpers
// ---------------------------------------------------------------------------

bool FUikaModule::LoadRustDll(const FString& LoadPath)
{
    DllHandle = FPlatformProcess::GetDllHandle(*LoadPath);
    if (!DllHandle)
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] Failed to load DLL: %s"), *LoadPath);
        return false;
    }
    CurrentLoadedDllPath = LoadPath;

    // Resolve entry points
    auto InitFn = reinterpret_cast<FUikaInitFn>(
        FPlatformProcess::GetDllExport(DllHandle, TEXT("uika_init")));
    if (!InitFn)
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] uika_init not found in DLL"));
        FPlatformProcess::FreeDllHandle(DllHandle);
        DllHandle = nullptr;
        return false;
    }

    // Initialize Rust side
    RustCallbacks = InitFn(&GApiTable);
    if (!RustCallbacks)
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] uika_init returned null"));
        FPlatformProcess::FreeDllHandle(DllHandle);
        DllHandle = nullptr;
        return false;
    }

    // Store globally so UUikaDelegateProxy can access Rust callbacks.
    GRustCallbacks = RustCallbacks;

    // Register the UObject delete listener for reified class instance cleanup.
    UikaReifyRegisterDeleteListener();

    return true;
}

void FUikaModule::UnloadRustDll()
{
    // Unregister delete listeners before shutting down Rust.
    UikaReifyUnregisterDeleteListener();
    UikaPinnedUnregisterDeleteListener();

    if (DllHandle)
    {
        // Notify Rust side
        if (RustCallbacks && RustCallbacks->on_shutdown)
        {
            RustCallbacks->on_shutdown();
        }

        // Call uika_shutdown if available
        auto ShutdownFn = reinterpret_cast<FUikaShutdownFn>(
            FPlatformProcess::GetDllExport(DllHandle, TEXT("uika_shutdown")));
        if (ShutdownFn)
        {
            ShutdownFn();
        }

        FPlatformProcess::FreeDllHandle(DllHandle);
        DllHandle = nullptr;
        RustCallbacks = nullptr;
        GRustCallbacks = nullptr;

        UE_LOG(LogUika, Display, TEXT("[Uika] Rust DLL unloaded."));
    }
}

// ---------------------------------------------------------------------------
// Hot reload
// ---------------------------------------------------------------------------

void FUikaModule::ReloadRustDll()
{
    UE_LOG(LogUika, Display, TEXT("[Uika] === Hot Reload Begin ==="));

    if (DllSourcePath.IsEmpty())
    {
        UE_LOG(LogUika, Error,
            TEXT("[Uika] Hot reload failed: DLL source path not set (was initial load skipped?)"));
        return;
    }

    // ------------------------------------------------------------------
    // Phase 1: Teardown — drop all Rust instances and unload old DLL
    // ------------------------------------------------------------------

    // 1a. Drop Rust instance data for all reified objects.
    if (DllHandle && RustCallbacks && RustCallbacks->drop_rust_instance)
    {
        int32 InstanceCount = 0;
        UikaReifyForEachReifiedInstance(
            [this, &InstanceCount](UObject* Obj, UUikaReifiedClass* ReifiedClass)
            {
                RustCallbacks->drop_rust_instance(
                    UikaUObjectHandle{ Obj },
                    ReifiedClass->RustTypeId,
                    nullptr);
                InstanceCount++;
            });
        UE_LOG(LogUika, Display,
            TEXT("[Uika] Hot reload: dropped %d Rust instances"), InstanceCount);
    }

    // 1b. Unload the old DLL (calls on_shutdown, uika_shutdown, FreeDllHandle).
    FString PreviousLoadedPath = CurrentLoadedDllPath;
    UnloadRustDll();

    // 1c. Delete previous hot-copy (now unlocked).
    if (!PreviousLoadedPath.IsEmpty() && PreviousLoadedPath != DllSourcePath)
    {
        IFileManager::Get().Delete(*PreviousLoadedPath, false, true, true);
    }

    // ------------------------------------------------------------------
    // Phase 2: Copy-on-reload — copy the new DLL to avoid Windows lock
    // ------------------------------------------------------------------

    if (!FPaths::FileExists(DllSourcePath))
    {
        UE_LOG(LogUika, Error,
            TEXT("[Uika] Hot reload failed: %s not found. Did cargo build succeed?"),
            *DllSourcePath);
        return;
    }

    ReloadCount++;
    const FString HotDllPath = FPaths::Combine(
        FPaths::GetPath(DllSourcePath),
        FString::Printf(TEXT("uika_hot_%d.dll"), ReloadCount));

    // Copy the freshly built DLL to a uniquely-named hot copy.
    uint32 CopyResult = IFileManager::Get().Copy(*HotDllPath, *DllSourcePath);
    if (CopyResult != 0)
    {
        UE_LOG(LogUika, Error,
            TEXT("[Uika] Hot reload failed: could not copy %s → %s (error %u)"),
            *DllSourcePath, *HotDllPath, CopyResult);
        return;
    }

    // ------------------------------------------------------------------
    // Phase 2b: Load the new DLL and re-initialize Rust
    // ------------------------------------------------------------------

    if (!LoadRustDll(HotDllPath))
    {
        UE_LOG(LogUika, Error, TEXT("[Uika] Hot reload failed: could not load new DLL"));
        return;
    }

    // ------------------------------------------------------------------
    // Phase 3: Reconstruct — rebuild Rust instance data for all objects
    // ------------------------------------------------------------------

    if (RustCallbacks && RustCallbacks->construct_rust_instance)
    {
        int32 ReconstructCount = 0;
        UikaReifyForEachReifiedInstance(
            [this, &ReconstructCount](UObject* Obj, UUikaReifiedClass* ReifiedClass)
            {
                bool bIsCDO = Obj->HasAnyFlags(RF_ClassDefaultObject);
                RustCallbacks->construct_rust_instance(
                    UikaUObjectHandle{ Obj },
                    ReifiedClass->RustTypeId,
                    bIsCDO);
                ReconstructCount++;
            });
        UE_LOG(LogUika, Display,
            TEXT("[Uika] Hot reload: reconstructed %d Rust instances"), ReconstructCount);
    }

    UE_LOG(LogUika, Display, TEXT("[Uika] === Hot Reload Complete ==="));
}

#undef LOCTEXT_NAMESPACE

IMPLEMENT_MODULE(FUikaModule, Uika)
