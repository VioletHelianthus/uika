// DynamicCall: reflection-based function invocation via ProcessEvent.
//
// This is the "safety net" fallback for functions not covered by codegen's
// direct call path. It uses UE's reflection system to find functions, allocate
// parameter buffers, set/get parameter values, and invoke via ProcessEvent.

use uika_ffi::{FPropertyHandle, UFunctionHandle, UObjectHandle};

use crate::api::api;
use crate::error::{check_ffi, UikaError, UikaResult};
use crate::object_ref::UObjectRef;
use crate::traits::UeClass;

/// Builder for a reflection-based function call.
///
/// Typical usage:
/// ```ignore
/// let mut call = DynamicCall::new(&obj, "DoSomething")?;
/// call.set::<i32>("Param1", 42)?;
/// let result = call.call()?;
/// let ret: f32 = result.get::<f32>("ReturnValue")?;
/// ```
pub struct DynamicCall {
    obj: UObjectHandle,
    func: UFunctionHandle,
    params: *mut u8,
}

impl DynamicCall {
    /// Prepare a reflection call to the named function on `obj`.
    pub fn new(obj: &UObjectRef<impl UeClass>, func_name: &str) -> UikaResult<Self> {
        let h = obj.checked()?.raw();
        let func = unsafe {
            ((*api().reflection).find_function)(h, func_name.as_ptr(), func_name.len() as u32)
        };
        if func.0.is_null() {
            return Err(UikaError::FunctionNotFound(func_name.to_string()));
        }
        let params = unsafe { ((*api().reflection).alloc_params)(func) };
        Ok(DynamicCall {
            obj: h,
            func,
            params,
        })
    }

    /// Write a parameter value into the params buffer.
    ///
    /// # Safety contract
    /// `T` must match the actual UE property type at the named parameter.
    /// Using the wrong type leads to undefined behavior at runtime. This is
    /// inherently less safe than the codegen direct-call path.
    pub fn set<T: Copy>(&mut self, name: &str, value: T) -> UikaResult<()> {
        let (prop, offset) = self.find_param(name)?;
        let _ = prop; // used only for lookup
        // SAFETY: The offset is provided by UE reflection and the caller
        // guarantees T matches the property type.
        unsafe {
            std::ptr::write_unaligned(self.params.add(offset as usize) as *mut T, value);
        }
        Ok(())
    }

    /// Invoke the function via ProcessEvent. Consumes this builder and returns
    /// a `DynamicCallResult` for reading output/return values.
    pub fn call(mut self) -> UikaResult<DynamicCallResult> {
        let code =
            unsafe { ((*api().reflection).call_function)(self.obj, self.func, self.params) };
        check_ffi(code)?;
        // Transfer params ownership to DynamicCallResult.
        let result = DynamicCallResult {
            func: self.func,
            params: self.params,
        };
        // Prevent Drop from double-freeing.
        self.params = std::ptr::null_mut();
        Ok(result)
    }

    /// Look up a named parameter and return its property handle + offset.
    fn find_param(&self, name: &str) -> UikaResult<(FPropertyHandle, u32)> {
        let prop = unsafe {
            ((*api().reflection).get_function_param)(self.func, name.as_ptr(), name.len() as u32)
        };
        if prop.0.is_null() {
            return Err(UikaError::PropertyNotFound(name.to_string()));
        }
        let offset = unsafe { ((*api().reflection).get_property_offset)(prop) };
        Ok((prop, offset))
    }
}

impl Drop for DynamicCall {
    fn drop(&mut self) {
        if !self.params.is_null() {
            unsafe { ((*api().reflection).free_params)(self.func, self.params) };
        }
    }
}

/// Holds the params buffer after a successful `DynamicCall::call()`.
/// Use `get()` to read output parameters and return values.
pub struct DynamicCallResult {
    func: UFunctionHandle,
    params: *mut u8,
}

impl DynamicCallResult {
    /// Read an output parameter or return value from the params buffer.
    ///
    /// # Safety contract
    /// `T` must match the actual UE property type at the named parameter.
    pub fn get<T: Copy>(&self, name: &str) -> UikaResult<T> {
        let prop = unsafe {
            ((*api().reflection).get_function_param)(self.func, name.as_ptr(), name.len() as u32)
        };
        if prop.0.is_null() {
            return Err(UikaError::PropertyNotFound(name.to_string()));
        }
        let offset = unsafe { ((*api().reflection).get_property_offset)(prop) };
        // SAFETY: The offset is provided by UE reflection and the caller
        // guarantees T matches the property type.
        let value = unsafe { std::ptr::read_unaligned(self.params.add(offset as usize) as *const T) };
        Ok(value)
    }
}

impl Drop for DynamicCallResult {
    fn drop(&mut self) {
        if !self.params.is_null() {
            unsafe { ((*api().reflection).free_params)(self.func, self.params) };
        }
    }
}
