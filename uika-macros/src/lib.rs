// uika-macros: proc macros for #[uclass], #[ufunction], #[uproperty].

mod prop_type;
mod uclass;
mod uclass_impl;

/// Attribute macro for defining a Rust struct as a UE class.
///
/// # Example
/// ```ignore
/// #[uclass(parent = Actor)]
/// pub struct MyEnemy {
///     #[uproperty(BlueprintReadWrite, default = 100.0)]
///     health: f32,
///
///     // Rust-private field (no #[uproperty])
///     chase_timer: f64,
/// }
/// ```
#[proc_macro_attribute]
pub fn uclass(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match uclass::expand_uclass(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Attribute macro for an impl block on a `#[uclass]` struct.
/// Methods marked `#[ufunction(...)]` become UE-callable functions.
///
/// # Example
/// ```ignore
/// #[uclass_impl]
/// impl MyEnemy {
///     #[ufunction(BlueprintCallable)]
///     fn take_damage(&mut self, amount: f32) -> bool {
///         let h = self.health() - amount;
///         self.set_health(h.max(0.0));
///         h <= 0.0
///     }
///
///     fn helper(&self) -> f32 { self.health() * 2.0 }  // plain Rust method
/// }
/// ```
#[proc_macro_attribute]
pub fn uclass_impl(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match uclass_impl::expand_uclass_impl(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
