// Widget creation and WidgetTree management (raw handle versions).
// Type-safe wrappers live in uika-bindings/src/manual/widget_ext.rs.

use uika_ffi::{UClassHandle, UObjectHandle};

use crate::error::{check_ffi, UikaError, UikaResult};
use crate::ffi_dispatch;

/// Create a UMG widget using the C++ `CreateWidget<T>()` template.
///
/// - `owning_object`: a PlayerController, World, or GameInstance handle
/// - `widget_class`: the UClass of the UUserWidget subclass to create
pub fn create_widget_raw(
    owning_object: UObjectHandle,
    widget_class: UClassHandle,
) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::widget_create_widget(owning_object, widget_class)
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation("create_widget returned null".into()))
    } else {
        Ok(result)
    }
}

/// Set the root widget of a UUserWidget's WidgetTree.
pub fn set_root_widget_raw(
    user_widget: UObjectHandle,
    root_widget: UObjectHandle,
) -> UikaResult<()> {
    check_ffi(unsafe {
        ffi_dispatch::widget_set_root_widget(user_widget, root_widget)
    })
}

/// Get the WidgetTree UObject from a UUserWidget.
pub fn get_widget_tree_raw(
    user_widget: UObjectHandle,
) -> UikaResult<UObjectHandle> {
    let result = unsafe {
        ffi_dispatch::widget_get_widget_tree(user_widget)
    };
    if result.is_null() {
        Err(UikaError::InvalidOperation("get_widget_tree returned null".into()))
    } else {
        Ok(result)
    }
}
