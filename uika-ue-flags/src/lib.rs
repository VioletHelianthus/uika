// UE reflection flag constants.
//
// Mirrors EPropertyFlags, EFunctionFlags, and EClassFlags from UE 5.7 source:
//   - Engine/Source/Runtime/CoreUObject/Public/UObject/ObjectMacros.h
//   - Engine/Source/Runtime/CoreUObject/Public/UObject/Script.h

// ---------------------------------------------------------------------------
// EPropertyFlags (CPF_*) — uint64
// ---------------------------------------------------------------------------

/// No flags.
pub const CPF_NONE: u64 = 0x0000_0000_0000_0000;
/// Property is user-settable in the editor.
pub const CPF_EDIT: u64 = 0x0000_0000_0000_0001;
/// This is a constant function parameter.
pub const CPF_CONST_PARM: u64 = 0x0000_0000_0000_0002;
/// This property can be read by blueprint code.
pub const CPF_BLUEPRINT_VISIBLE: u64 = 0x0000_0000_0000_0004;
/// Object can be exported with actor.
pub const CPF_EXPORT_OBJECT: u64 = 0x0000_0000_0000_0008;
/// This property cannot be modified by blueprint code.
pub const CPF_BLUEPRINT_READ_ONLY: u64 = 0x0000_0000_0000_0010;
/// Property is relevant to network replication.
pub const CPF_NET: u64 = 0x0000_0000_0000_0020;
/// Elements of an array can be modified, but its size cannot be changed.
pub const CPF_EDIT_FIXED_SIZE: u64 = 0x0000_0000_0000_0040;
/// Function/When call parameter.
pub const CPF_PARM: u64 = 0x0000_0000_0000_0080;
/// Value is copied out after function call.
pub const CPF_OUT_PARM: u64 = 0x0000_0000_0000_0100;
/// memset is fine for construction.
pub const CPF_ZERO_CONSTRUCTOR: u64 = 0x0000_0000_0000_0200;
/// Return value.
pub const CPF_RETURN_PARM: u64 = 0x0000_0000_0000_0400;
/// Disable editing of this property on an archetype/sub-blueprint.
pub const CPF_DISABLE_EDIT_ON_TEMPLATE: u64 = 0x0000_0000_0000_0800;
/// Object property can never be null.
pub const CPF_NON_NULLABLE: u64 = 0x0000_0000_0000_1000;
/// Property is transient: shouldn't be saved or loaded, except for Blueprint CDOs.
pub const CPF_TRANSIENT: u64 = 0x0000_0000_0000_2000;
/// Property should be loaded/saved as permanent profile.
pub const CPF_CONFIG: u64 = 0x0000_0000_0000_4000;
/// Parameter must be linked explicitly in blueprint.
pub const CPF_REQUIRED_PARM: u64 = 0x0000_0000_0000_8000;
/// Disable editing on an instance of this class.
pub const CPF_DISABLE_EDIT_ON_INSTANCE: u64 = 0x0000_0000_0001_0000;
/// Property is uneditable in the editor.
pub const CPF_EDIT_CONST: u64 = 0x0000_0000_0002_0000;
/// Load config from base class, not subclass.
pub const CPF_GLOBAL_CONFIG: u64 = 0x0000_0000_0004_0000;
/// Property is a component reference.
pub const CPF_INSTANCED_REFERENCE: u64 = 0x0000_0000_0008_0000;
/// Property saves objects in separate files, breaks hard links and reload them based on discovery.
pub const CPF_EXPERIMENTAL_EXTERNAL_OBJECTS: u64 = 0x0000_0000_0010_0000;
/// Property should always be reset to the default value during any type of duplication.
pub const CPF_DUPLICATE_TRANSIENT: u64 = 0x0000_0000_0020_0000;
// 0x0000_0000_0040_0000 — reserved
// 0x0000_0000_0080_0000 — reserved
/// Property should be serialized for save games.
pub const CPF_SAVE_GAME: u64 = 0x0000_0000_0100_0000;
/// Hide clear button.
pub const CPF_NO_CLEAR: u64 = 0x0000_0000_0200_0000;
/// Property is defined on an interface and does not include a useful Offset_Internal.
pub const CPF_VIRTUAL: u64 = 0x0000_0000_0400_0000;
/// Value is passed by reference; CPF_OUT_PARM and CPF_PARM should also be set.
pub const CPF_REFERENCE_PARM: u64 = 0x0000_0000_0800_0000;
/// MC Delegates only. Property should be exposed for assigning in blueprint code.
pub const CPF_BLUEPRINT_ASSIGNABLE: u64 = 0x0000_0000_1000_0000;
/// Property is deprecated. Read it from an archive, but don't save it.
pub const CPF_DEPRECATED: u64 = 0x0000_0000_2000_0000;
/// Property can be memcopied instead of CopyCompleteValue / CopySingleValue.
pub const CPF_IS_PLAIN_OLD_DATA: u64 = 0x0000_0000_4000_0000;
/// Not replicated. For non replicated properties in replicated structs.
pub const CPF_REP_SKIP: u64 = 0x0000_0000_8000_0000;
/// Notify actors when a property is replicated.
pub const CPF_REP_NOTIFY: u64 = 0x0000_0001_0000_0000;
/// Interpolatable property for use with cinematics.
pub const CPF_INTERP: u64 = 0x0000_0002_0000_0000;
/// Property isn't transacted.
pub const CPF_NON_TRANSACTIONAL: u64 = 0x0000_0004_0000_0000;
/// Property should only be loaded in the editor.
pub const CPF_EDITOR_ONLY: u64 = 0x0000_0008_0000_0000;
/// No destructor.
pub const CPF_NO_DESTRUCTOR: u64 = 0x0000_0010_0000_0000;
// 0x0000_0020_0000_0000 — reserved
/// Only used for weak pointers, means the export type is autoweak.
pub const CPF_AUTO_WEAK: u64 = 0x0000_0040_0000_0000;
/// Property contains component references.
pub const CPF_CONTAINS_INSTANCED_REFERENCE: u64 = 0x0000_0080_0000_0000;
/// Asset instances will add properties with this flag to the asset registry automatically.
pub const CPF_ASSET_REGISTRY_SEARCHABLE: u64 = 0x0000_0100_0000_0000;
/// The property is visible by default in the editor details view.
pub const CPF_SIMPLE_DISPLAY: u64 = 0x0000_0200_0000_0000;
/// The property is advanced and not visible by default in the editor details view.
pub const CPF_ADVANCED_DISPLAY: u64 = 0x0000_0400_0000_0000;
/// Property is protected from the perspective of script.
pub const CPF_PROTECTED: u64 = 0x0000_0800_0000_0000;
/// MC Delegates only. Property should be exposed for calling in blueprint code.
pub const CPF_BLUEPRINT_CALLABLE: u64 = 0x0000_1000_0000_0000;
/// MC Delegates only. This delegate accepts (only in blueprint) only events with BlueprintAuthorityOnly.
pub const CPF_BLUEPRINT_AUTHORITY_ONLY: u64 = 0x0000_2000_0000_0000;
/// Property shouldn't be exported to text format (e.g. copy/paste).
pub const CPF_TEXT_EXPORT_TRANSIENT: u64 = 0x0000_4000_0000_0000;
/// Property should only be copied in PIE.
pub const CPF_NON_PIE_DUPLICATE_TRANSIENT: u64 = 0x0000_8000_0000_0000;
/// Property is exposed on spawn.
pub const CPF_EXPOSE_ON_SPAWN: u64 = 0x0001_0000_0000_0000;
/// A object referenced by the property is duplicated like a component.
pub const CPF_PERSISTENT_INSTANCE: u64 = 0x0002_0000_0000_0000;
/// Property was parsed as a wrapper class like TSubclassOf<T>, FScriptInterface etc.
pub const CPF_UOBJECT_WRAPPER: u64 = 0x0004_0000_0000_0000;
/// This property can generate a meaningful hash value.
pub const CPF_HAS_GET_VALUE_TYPE_HASH: u64 = 0x0008_0000_0000_0000;
/// Public native access specifier.
pub const CPF_NATIVE_ACCESS_SPECIFIER_PUBLIC: u64 = 0x0010_0000_0000_0000;
/// Protected native access specifier.
pub const CPF_NATIVE_ACCESS_SPECIFIER_PROTECTED: u64 = 0x0020_0000_0000_0000;
/// Private native access specifier.
pub const CPF_NATIVE_ACCESS_SPECIFIER_PRIVATE: u64 = 0x0040_0000_0000_0000;
/// Property shouldn't be serialized, can still be exported to text.
pub const CPF_SKIP_SERIALIZATION: u64 = 0x0080_0000_0000_0000;
/// Property is a TObjectPtr<T> instead of a USomething*.
pub const CPF_TOBJECT_PTR: u64 = 0x0100_0000_0000_0000;
/// [Experimental] Property will use overridable serialization logic.
pub const CPF_EXPERIMENTAL_OVERRIDABLE_LOGIC: u64 = 0x0200_0000_0000_0000;
/// [Experimental] Property should never inherit from the parent when using overridable serialization.
pub const CPF_EXPERIMENTAL_ALWAYS_OVERRIDEN: u64 = 0x0400_0000_0000_0000;
/// [Experimental] Property should never be overridden when using overridable serialization.
pub const CPF_EXPERIMENTAL_NEVER_OVERRIDEN: u64 = 0x0800_0000_0000_0000;
/// Enables the instancing graph self referencing logic.
pub const CPF_ALLOW_SELF_REFERENCE: u64 = 0x1000_0000_0000_0000;

// Combined CPF masks

/// All native access specifier flags.
pub const CPF_NATIVE_ACCESS_SPECIFIERS: u64 =
    CPF_NATIVE_ACCESS_SPECIFIER_PUBLIC | CPF_NATIVE_ACCESS_SPECIFIER_PROTECTED | CPF_NATIVE_ACCESS_SPECIFIER_PRIVATE;

/// All parameter flags.
pub const CPF_PARM_FLAGS: u64 =
    CPF_PARM | CPF_OUT_PARM | CPF_RETURN_PARM | CPF_REQUIRED_PARM | CPF_REFERENCE_PARM | CPF_CONST_PARM;

// ---------------------------------------------------------------------------
// EFunctionFlags (FUNC_*) — uint32
// ---------------------------------------------------------------------------

/// No flags.
pub const FUNC_NONE: u32 = 0x0000_0000;
/// Function is final (prebindable, non-overridable function).
pub const FUNC_FINAL: u32 = 0x0000_0001;
/// Indicates this function is DLL exported/imported.
pub const FUNC_REQUIRED_API: u32 = 0x0000_0002;
/// Function will only run if the object has network authority.
pub const FUNC_BLUEPRINT_AUTHORITY_ONLY: u32 = 0x0000_0004;
/// Function is cosmetic in nature and should not be invoked on dedicated servers.
pub const FUNC_BLUEPRINT_COSMETIC: u32 = 0x0000_0008;
// 0x0000_0010 — reserved
// 0x0000_0020 — reserved
/// Function is network-replicated.
pub const FUNC_NET: u32 = 0x0000_0040;
/// Function should be sent reliably on the network.
pub const FUNC_NET_RELIABLE: u32 = 0x0000_0080;
/// Function is sent to a net service.
pub const FUNC_NET_REQUEST: u32 = 0x0000_0100;
/// Executable from command line.
pub const FUNC_EXEC: u32 = 0x0000_0200;
/// Native function.
pub const FUNC_NATIVE: u32 = 0x0000_0400;
/// Event function.
pub const FUNC_EVENT: u32 = 0x0000_0800;
/// Function response from a net service.
pub const FUNC_NET_RESPONSE: u32 = 0x0000_1000;
/// Static function.
pub const FUNC_STATIC: u32 = 0x0000_2000;
/// Function is networked multicast Server -> All Clients.
pub const FUNC_NET_MULTICAST: u32 = 0x0000_4000;
/// Function is used as the merge 'ubergraph' for a blueprint.
pub const FUNC_UBERGRAPH_FUNCTION: u32 = 0x0000_8000;
/// Function is a multi-cast delegate signature (also requires FUNC_DELEGATE).
pub const FUNC_MULTICAST_DELEGATE: u32 = 0x0001_0000;
/// Function is accessible in all classes.
pub const FUNC_PUBLIC: u32 = 0x0002_0000;
/// Function is accessible only in the class it is defined in.
pub const FUNC_PRIVATE: u32 = 0x0004_0000;
/// Function is accessible only in the class it is defined in and subclasses.
pub const FUNC_PROTECTED: u32 = 0x0008_0000;
/// Function is delegate signature (single-cast or multi-cast).
pub const FUNC_DELEGATE: u32 = 0x0010_0000;
/// Function is executed on servers.
pub const FUNC_NET_SERVER: u32 = 0x0020_0000;
/// Function has out (pass by reference) parameters.
pub const FUNC_HAS_OUT_PARMS: u32 = 0x0040_0000;
/// Function has structs that contain defaults.
pub const FUNC_HAS_DEFAULTS: u32 = 0x0080_0000;
/// Function is executed on clients.
pub const FUNC_NET_CLIENT: u32 = 0x0100_0000;
/// Function is imported from a DLL.
pub const FUNC_DLL_IMPORT: u32 = 0x0200_0000;
/// Function can be called from blueprint code.
pub const FUNC_BLUEPRINT_CALLABLE: u32 = 0x0400_0000;
/// Function can be overridden/implemented from a blueprint.
pub const FUNC_BLUEPRINT_EVENT: u32 = 0x0800_0000;
/// Function can be called from blueprint code, and is also pure (no side effects).
pub const FUNC_BLUEPRINT_PURE: u32 = 0x1000_0000;
/// Function can only be called from an editor script.
pub const FUNC_EDITOR_ONLY: u32 = 0x2000_0000;
/// Function can be called from blueprint code, and only reads state (never writes state).
pub const FUNC_CONST: u32 = 0x4000_0000;
/// Function must supply a _Validate implementation.
pub const FUNC_NET_VALIDATE: u32 = 0x8000_0000;
/// All flags.
pub const FUNC_ALL_FLAGS: u32 = 0xFFFF_FFFF;

// ---------------------------------------------------------------------------
// EClassFlags (CLASS_*) — uint32
// ---------------------------------------------------------------------------

/// No flags.
pub const CLASS_NONE: u32 = 0x0000_0000;
/// Class is abstract and can't be instantiated directly.
pub const CLASS_ABSTRACT: u32 = 0x0000_0001;
/// Save object configuration only to Default INIs, never to local INIs.
pub const CLASS_DEFAULT_CONFIG: u32 = 0x0000_0002;
/// Load object configuration at construction time.
pub const CLASS_CONFIG: u32 = 0x0000_0004;
/// This object type can't be saved; null it out at save time.
pub const CLASS_TRANSIENT: u32 = 0x0000_0008;
/// This object type may not be available in certain context.
pub const CLASS_OPTIONAL: u32 = 0x0000_0010;
/// Matched serializers.
pub const CLASS_MATCHED_SERIALIZERS: u32 = 0x0000_0020;
/// Config settings for this class will be saved to Project/User*.ini.
pub const CLASS_PROJECT_USER_CONFIG: u32 = 0x0000_0040;
/// Class is a native class.
pub const CLASS_NATIVE: u32 = 0x0000_0080;
// 0x0000_0100 — reserved
/// Do not allow users to create in the editor.
pub const CLASS_NOT_PLACEABLE: u32 = 0x0000_0200;
/// Handle object configuration on a per-object basis, rather than per-class.
pub const CLASS_PER_OBJECT_CONFIG: u32 = 0x0000_0400;
/// Whether SetUpRuntimeReplicationData still needs to be called for this class.
pub const CLASS_REPLICATION_DATA_IS_SET_UP: u32 = 0x0000_0800;
/// Class can be constructed from editinline New button.
pub const CLASS_EDIT_INLINE_NEW: u32 = 0x0000_1000;
/// Display properties in the editor without using categories.
pub const CLASS_COLLAPSE_CATEGORIES: u32 = 0x0000_2000;
/// Class is an interface.
pub const CLASS_INTERFACE: u32 = 0x0000_4000;
/// Config for this class is overridden in platform inis.
pub const CLASS_PER_PLATFORM_CONFIG: u32 = 0x0000_8000;
/// All properties and functions in this class are const.
pub const CLASS_CONST: u32 = 0x0001_0000;
/// Class flag indicating objects of this class need deferred dependency loading.
pub const CLASS_NEEDS_DEFERRED_DEPENDENCY_LOADING: u32 = 0x0002_0000;
/// Indicates that the class was created from blueprint source material.
pub const CLASS_COMPILED_FROM_BLUEPRINT: u32 = 0x0004_0000;
/// Indicates that only the bare minimum bits of this class should be DLL exported/imported.
pub const CLASS_MINIMAL_API: u32 = 0x0008_0000;
/// Indicates this class must be DLL exported/imported (along with all of its members).
pub const CLASS_REQUIRED_API: u32 = 0x0010_0000;
/// Indicates that references to this class default to instanced.
pub const CLASS_DEFAULT_TO_INSTANCED: u32 = 0x0020_0000;
/// Indicates that the parent token stream has been merged with ours.
pub const CLASS_TOKEN_STREAM_ASSEMBLED: u32 = 0x0040_0000;
/// Class has component properties.
pub const CLASS_HAS_INSTANCED_REFERENCE: u32 = 0x0080_0000;
/// Don't show this class in the editor class browser or edit inline new menus.
pub const CLASS_HIDDEN: u32 = 0x0100_0000;
/// Don't save objects of this class when serializing.
pub const CLASS_DEPRECATED: u32 = 0x0200_0000;
/// Class not shown in editor drop down for class selection.
pub const CLASS_HIDE_DROP_DOWN: u32 = 0x0400_0000;
/// Class settings are saved to AppData (as opposed to CLASS_DEFAULT_CONFIG).
pub const CLASS_GLOBAL_USER_CONFIG: u32 = 0x0800_0000;
/// Class was declared directly in C++ and has no boilerplate generated by UnrealHeaderTool.
pub const CLASS_INTRINSIC: u32 = 0x1000_0000;
/// Class has already been constructed (maybe in a previous DLL version before hot-reload).
pub const CLASS_CONSTRUCTED: u32 = 0x2000_0000;
/// Indicates that object configuration will not check against ini base/defaults when serialized.
pub const CLASS_CONFIG_DO_NOT_CHECK_DEFAULTS: u32 = 0x4000_0000;
/// Class has been consigned to oblivion as part of a blueprint recompile; newer version exists.
pub const CLASS_NEWER_VERSION_EXISTS: u32 = 0x8000_0000;

/// Flags inherited from base class.
pub const CLASS_INHERIT: u32 = CLASS_TRANSIENT
    | CLASS_OPTIONAL
    | CLASS_DEFAULT_CONFIG
    | CLASS_CONFIG
    | CLASS_PER_OBJECT_CONFIG
    | CLASS_CONFIG_DO_NOT_CHECK_DEFAULTS
    | CLASS_NOT_PLACEABLE
    | CLASS_CONST
    | CLASS_HAS_INSTANCED_REFERENCE
    | CLASS_DEPRECATED
    | CLASS_DEFAULT_TO_INSTANCED
    | CLASS_GLOBAL_USER_CONFIG
    | CLASS_PROJECT_USER_CONFIG
    | CLASS_PER_PLATFORM_CONFIG
    | CLASS_NEEDS_DEFERRED_DEPENDENCY_LOADING;
