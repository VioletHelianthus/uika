// Type-safe UMG widget helpers on top of uika_runtime::widget raw functions.

use uika_runtime::{UObjectRef, UeClass, UikaResult};

/// Create a UMG widget of type `T` (must be a UUserWidget subclass).
///
/// `owner` should be a PlayerController, World, or GameInstance.
pub fn create_widget<T: UeClass>(
    owner: &UObjectRef<impl UeClass>,
) -> UikaResult<UObjectRef<T>> {
    let owner_handle = owner.checked()?.raw();
    let class = T::static_class();
    let handle = uika_runtime::widget::create_widget_raw(owner_handle, class)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Create a child widget of type `T`, using the given UUserWidget's WidgetTree as outer.
///
/// This is useful for programmatic widget tree construction where child widgets
/// need to be owned by the parent widget's WidgetTree.
pub fn create_child_widget<T: UeClass>(
    parent_user_widget: &UObjectRef<impl UeClass>,
) -> UikaResult<UObjectRef<T>> {
    let parent_handle = parent_user_widget.checked()?.raw();
    let tree_handle = uika_runtime::widget::get_widget_tree_raw(parent_handle)?;
    let class = T::static_class();
    let handle = uika_runtime::world::new_object_raw(tree_handle, class)?;
    Ok(unsafe { UObjectRef::from_raw(handle) })
}

/// Set the root widget of a UUserWidget's WidgetTree.
pub fn set_root_widget(
    user_widget: &UObjectRef<impl UeClass>,
    root_widget: &UObjectRef<impl UeClass>,
) -> UikaResult<()> {
    let uw_handle = user_widget.checked()?.raw();
    let rw_handle = root_widget.checked()?.raw();
    uika_runtime::widget::set_root_widget_raw(uw_handle, rw_handle)
}

/// Get the WidgetTree from a UUserWidget.
pub fn get_widget_tree(
    user_widget: &UObjectRef<impl UeClass>,
) -> UikaResult<uika_ffi::UObjectHandle> {
    let handle = user_widget.checked()?.raw();
    uika_runtime::widget::get_widget_tree_raw(handle)
}
