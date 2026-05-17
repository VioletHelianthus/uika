// UikaLifecycleApiImpl.cpp — FUikaLifecycleApi implementation.
//
// Provides GC root management and Pinned object destroy notification.
// - add_gc_root / remove_gc_root: prevent/allow UE garbage collection
// - register_pinned / unregister_pinned: track Pinned objects for destroy notification

#include "UikaApiTable.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UObjectArray.h"

// Access to Rust callbacks (defined in UikaModule.cpp).
extern const FUikaRustCallbacks* GetUikaRustCallbacks();

// ---------------------------------------------------------------------------
// GC root management
// ---------------------------------------------------------------------------

static void AddGcRootImpl(UikaUObjectHandle Obj)
{
    UObject* Object = static_cast<UObject*>(Obj.ptr);
    if (::IsValid(Object))
    {
        Object->AddToRoot();
    }
}

static void RemoveGcRootImpl(UikaUObjectHandle Obj)
{
    UObject* Object = static_cast<UObject*>(Obj.ptr);
    if (::IsValid(Object))
    {
        Object->RemoveFromRoot();
    }
}

// ---------------------------------------------------------------------------
// Pinned object tracking + destroy notification
// ---------------------------------------------------------------------------

// Set of UObject pointers that have active Pinned<T> handles in Rust.
// Checked by the delete listener to fire notify_pinned_destroyed.
static TSet<const UObjectBase*> GPinnedObjects;

// Delete listener that watches GUObjectArray for pinned object destruction.
// Extends the existing FUikaDeleteListener pattern from UikaReifyApiImpl.cpp.
class FUikaPinnedDeleteListener : public FUObjectArray::FUObjectDeleteListener
{
public:
    virtual void NotifyUObjectDeleted(const UObjectBase* Object, int32 Index) override
    {
        if (!GPinnedObjects.Contains(Object))
        {
            return;
        }

        // Notify Rust that this pinned object has been destroyed.
        const FUikaRustCallbacks* Callbacks = GetUikaRustCallbacks();
        if (Callbacks && Callbacks->notify_pinned_destroyed)
        {
            Callbacks->notify_pinned_destroyed(
                UikaUObjectHandle{ const_cast<UObjectBase*>(Object) });
        }

        // Remove from tracking — the Pinned<T> drop will call unregister_pinned
        // but the object is already gone, so we clean up proactively.
        GPinnedObjects.Remove(Object);
    }

    virtual void OnUObjectArrayShutdown() override
    {
        GUObjectArray.RemoveUObjectDeleteListener(this);
    }
};

static FUikaPinnedDeleteListener GPinnedDeleteListener;
static bool GPinnedListenerRegistered = false;

static void EnsurePinnedListenerRegistered()
{
    if (!GPinnedListenerRegistered)
    {
        GUObjectArray.AddUObjectDeleteListener(&GPinnedDeleteListener);
        GPinnedListenerRegistered = true;
    }
}

static void RegisterPinnedImpl(UikaUObjectHandle Obj)
{
    const UObjectBase* Object = static_cast<const UObjectBase*>(Obj.ptr);
    if (Object)
    {
        EnsurePinnedListenerRegistered();
        GPinnedObjects.Add(Object);
    }
}

static void UnregisterPinnedImpl(UikaUObjectHandle Obj)
{
    const UObjectBase* Object = static_cast<const UObjectBase*>(Obj.ptr);
    if (Object)
    {
        GPinnedObjects.Remove(Object);
    }
}

// Called from UikaModule.cpp during DLL unload to clean up.
//
// Hot reload note: when the Rust DLL is unloaded, user statics holding
// Pinned<T> values are forgotten without their Drop running, so Rust never
// calls remove_gc_root for them. Without this loop those objects stay in
// UE's root set forever and trip the !IsRooted() assertion when PIE later
// tries to clean up the world. We mirror what Pinned<T>::drop would have
// done on the C++ side.
void UikaPinnedUnregisterDeleteListener()
{
    if (GPinnedListenerRegistered)
    {
        GUObjectArray.RemoveUObjectDeleteListener(&GPinnedDeleteListener);
        GPinnedListenerRegistered = false;
    }
    for (const UObjectBase* TrackedBase : GPinnedObjects)
    {
        UObject* Object = static_cast<UObject*>(const_cast<UObjectBase*>(TrackedBase));
        if (::IsValid(Object) && Object->IsRooted())
        {
            Object->RemoveFromRoot();
        }
    }
    GPinnedObjects.Empty();
}

// ---------------------------------------------------------------------------
// Static instance
// ---------------------------------------------------------------------------

FUikaLifecycleApi GLifecycleApi = {
    &AddGcRootImpl,
    &RemoveGcRootImpl,
    &RegisterPinnedImpl,
    &UnregisterPinnedImpl,
};
