//! WasmHost: manages wasmtime engine, store, and instance.

use std::ffi::c_void;

use uika_ffi::api_table::UikaApiTable;
use uika_ffi::handles::*;
use wasmtime::*;

/// State accessible from within host functions via `Caller<HostState>`.
pub struct HostState {
    /// Pointer to the C++ API table (valid for the lifetime of the DLL).
    pub api: *const UikaApiTable,
    /// WASM linear memory — set after instantiation.
    pub memory: Option<Memory>,
}

// Safety: all WASM execution happens on the game thread only.
unsafe impl Send for HostState {}

// Auto-generated host function registration from api_table.rs
#[allow(unused_imports, unused_variables, unused_mut, clippy::all)]
mod host_functions_generated {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/host_functions_generated.rs"));
}
use host_functions_generated::register_all_host_functions;

// Codegen-generated host function registration (~3,700 functions from UHT metadata)
#[allow(unused_imports, unused_variables, unused_mut, clippy::all)]
mod codegen_host_funcs {
    use super::*;
    include!("generated/codegen_host_funcs.rs");
}

/// Create a wasmtime Engine with platform-appropriate configuration.
/// iOS: Cranelift compiles to Pulley bytecode, Pulley interprets at runtime.
/// Other platforms: Cranelift compiles to native code, direct execution.
fn create_engine() -> wasmtime::Result<Engine> {
    #[allow(unused_mut)]
    let mut config = Config::new();
    #[cfg(target_os = "ios")]
    config.target("pulley64")?;
    Engine::new(&config)
}

/// Manages the wasmtime engine, store, and instance.
pub struct WasmHost {
    store: Store<HostState>,
    instance: Instance,
}

impl WasmHost {
    /// Create a new WasmHost from raw WASM bytes and the UE API table pointer.
    pub fn new(api: *const UikaApiTable, wasm_bytes: &[u8]) -> Result<Self> {
        // Run initialization on a dedicated thread with a large stack.
        // UE's game thread has a limited stack size (~1 MB) which is not enough
        // for wasmtime's Cranelift JIT compilation + registering ~3,800 host functions.
        let api_raw = api as usize;
        let bytes = wasm_bytes.to_vec();
        let result = std::thread::Builder::new()
            .name("wasm-init".into())
            .stack_size(16 * 1024 * 1024) // 16 MB
            .spawn(move || -> Result<Self> {
                let api = api_raw as *const UikaApiTable;
                Self::do_new(api, &bytes)
            })
            .expect("failed to spawn wasm-init thread")
            .join()
            .map_err(|_| wasmtime::Error::msg("wasm-init thread panicked"))?;
        result
    }

    fn do_new(api: *const UikaApiTable, wasm_bytes: &[u8]) -> Result<Self> {
        let engine = create_engine()?;
        crate::ue_log(0, "[WasmHost] Compiling WASM module...");
        let module = Module::new(&engine, wasm_bytes)?;

        let mut linker = Linker::new(&engine);

        // Register auto-generated host functions (~102 sub-table functions)
        crate::ue_log(0, "[WasmHost] Registering sub-table host functions...");
        register_all_host_functions(&mut linker)?;

        // Register hand-written host functions
        crate::ue_log(0, "[WasmHost] Registering manual host functions...");
        register_manual_host_functions(&mut linker)?;

        // Register codegen host functions (~3,700 UHT-generated functions)
        crate::ue_log(0, "[WasmHost] Registering codegen host functions...");
        codegen_host_funcs::register_codegen_host_functions(&mut linker)?;

        crate::ue_log(0, "[WasmHost] Instantiating WASM module...");
        let state = HostState { api, memory: None };
        let mut store = Store::new(&engine, state);

        let instance = linker.instantiate(&mut store, &module)?;

        // Capture the exported "memory"
        if let Some(Extern::Memory(mem)) = instance.get_export(&mut store, "memory") {
            store.data_mut().memory = Some(mem);
        }

        Ok(WasmHost { store, instance })
    }

    /// Call WASM initialization: `__wasm_call_ctors` (if present), then `uika_wasm_init`.
    pub fn call_init(&mut self) -> Result<()> {
        // Some toolchains emit __wasm_call_ctors for static constructors (inventory)
        if let Ok(ctor) = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "__wasm_call_ctors")
        {
            ctor.call(&mut self.store, ())?;
        }

        let init = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "uika_wasm_init")?;
        init.call(&mut self.store, ())?;
        Ok(())
    }

    /// Forward a delegate callback into WASM.
    pub fn invoke_delegate(&mut self, callback_id: u64, params: i64) -> Result<()> {
        let func = self.instance.get_typed_func::<(i64, i64), ()>(
            &mut self.store,
            "uika_invoke_delegate",
        )?;
        func.call(&mut self.store, (callback_id as i64, params))?;
        Ok(())
    }

    /// Forward a UFunction invocation into WASM.
    pub fn invoke_function(
        &mut self,
        callback_id: u64,
        obj: i64,
        params: i64,
    ) -> Result<()> {
        let func = self.instance.get_typed_func::<(i64, i64, i64), ()>(
            &mut self.store,
            "uika_invoke_function",
        )?;
        func.call(
            &mut self.store,
            (callback_id as i64, obj, params),
        )?;
        Ok(())
    }

    /// Forward construct_instance into WASM.
    pub fn construct_instance(
        &mut self,
        obj: i64,
        type_id: i64,
        is_cdo: i32,
    ) -> Result<()> {
        let func = self.instance.get_typed_func::<(i64, i64, i32), ()>(
            &mut self.store,
            "uika_construct_instance",
        )?;
        func.call(&mut self.store, (obj, type_id, is_cdo))?;
        Ok(())
    }

    /// Forward drop_instance into WASM.
    pub fn drop_instance(&mut self, obj: i64, type_id: i64) -> Result<()> {
        let func = self.instance.get_typed_func::<(i64, i64), ()>(
            &mut self.store,
            "uika_drop_instance",
        )?;
        func.call(&mut self.store, (obj, type_id))?;
        Ok(())
    }

    /// Notify WASM that a pinned object was destroyed.
    pub fn notify_pinned_destroyed(&mut self, obj: i64) -> Result<()> {
        let func = self.instance.get_typed_func::<(i64,), ()>(
            &mut self.store,
            "uika_notify_pinned_destroyed",
        )?;
        func.call(&mut self.store, (obj,))?;
        Ok(())
    }

    /// Call the on_shutdown export.
    pub fn on_shutdown(&mut self) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "uika_on_shutdown")?;
        func.call(&mut self.store, ())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WASM memory helpers (used by generated and manual host functions)
// ---------------------------------------------------------------------------

/// Read bytes from WASM linear memory.
pub fn read_guest_bytes(caller: &Caller<'_, HostState>, ptr: u32, len: u32) -> Vec<u8> {
    let Some(memory) = caller.data().memory else {
        return Vec::new();
    };
    let data = memory.data(caller);
    let start = ptr as usize;
    let end = start.saturating_add(len as usize);
    if end > data.len() {
        return Vec::new();
    }
    data[start..end].to_vec()
}

/// Write bytes to WASM linear memory.
pub fn write_guest_bytes(caller: &mut Caller<'_, HostState>, ptr: u32, bytes: &[u8]) {
    let Some(memory) = caller.data().memory else {
        return;
    };
    let data = memory.data_mut(caller);
    let start = ptr as usize;
    let end = start.saturating_add(bytes.len());
    if end <= data.len() {
        data[start..end].copy_from_slice(bytes);
    }
}

// ---------------------------------------------------------------------------
// Manual host functions (not auto-generated from api_table.rs)
// ---------------------------------------------------------------------------

fn register_manual_host_functions(linker: &mut Linker<HostState>) -> Result<()> {
    // Read native memory: guest sends a native pointer + offset + length,
    // host reads from native memory and writes to WASM memory.
    // Used by DynamicCall on wasm32 to read from alloc_params buffers.
    // Guest import: fn uika_read_native_mem(src: i64, dst: i32, len: i32)
    // Guest pre-adds offset to ptr, so host receives the final address.
    linker.func_wrap(
        "uika",
        "uika_read_native_mem",
        |mut caller: Caller<'_, HostState>,
         src: i64,
         wasm_dst: i32,
         len: i32| {
            let ptr = src as usize as *const u8;
            let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
            write_guest_bytes(&mut caller, wasm_dst as u32, bytes);
        },
    )?;

    // Guest import: fn uika_write_native_mem(dst: i64, src: i32, len: i32)
    // Guest pre-adds offset to ptr, so host receives the final address.
    linker.func_wrap(
        "uika",
        "uika_write_native_mem",
        |caller: Caller<'_, HostState>,
         dst: i64,
         wasm_src: i32,
         len: i32| {
            let bytes = read_guest_bytes(&caller, wasm_src as u32, len as u32);
            let ptr = dst as usize as *mut u8;
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            }
        },
    )?;

    // Allocate a native-memory struct: alloc + initialize_struct, return native pointer.
    // Guest import: fn uika_struct_alloc(ustruct: i64) -> i64
    linker.func_wrap(
        "uika",
        "uika_struct_alloc",
        |caller: Caller<'_, HostState>, ustruct: i64| -> i64 {
            let api = caller.data().api;
            unsafe {
                let refl = &*(*api).reflection;
                let sh = UStructHandle(ustruct as usize as *mut std::ffi::c_void);
                let size = (refl.get_struct_size)(sh);
                let layout = std::alloc::Layout::from_size_align(size as usize, 16).unwrap();
                let ptr = std::alloc::alloc_zeroed(layout);
                (refl.initialize_struct)(sh, ptr);
                ptr as usize as i64
            }
        },
    )?;

    // Free a native-memory struct: destroy_struct + dealloc.
    // Guest import: fn uika_struct_free(ustruct: i64, ptr: i64)
    linker.func_wrap(
        "uika",
        "uika_struct_free",
        |caller: Caller<'_, HostState>, ustruct: i64, ptr: i64| {
            let api = caller.data().api;
            unsafe {
                let refl = &*(*api).reflection;
                let sh = UStructHandle(ustruct as usize as *mut std::ffi::c_void);
                let native_ptr = ptr as usize as *mut u8;
                (refl.destroy_struct)(sh, native_ptr);
                let size = (refl.get_struct_size)(sh);
                let layout = std::alloc::Layout::from_size_align(size as usize, 16).unwrap();
                std::alloc::dealloc(native_ptr, layout);
            }
        },
    )?;

    // Generic native memory alloc/free (for handle arrays, etc.)
    // Guest import: fn uika_native_alloc(size: i32) -> i64
    linker.func_wrap(
        "uika",
        "uika_native_alloc",
        |_caller: Caller<'_, HostState>, size: i32| -> i64 {
            unsafe {
                let layout = std::alloc::Layout::from_size_align(size as usize, 16).unwrap();
                let ptr = std::alloc::alloc_zeroed(layout);
                ptr as usize as i64
            }
        },
    )?;

    // Guest import: fn uika_native_free(ptr: i64, size: i32)
    linker.func_wrap(
        "uika",
        "uika_native_free",
        |_caller: Caller<'_, HostState>, ptr: i64, size: i32| {
            unsafe {
                let layout = std::alloc::Layout::from_size_align(size as usize, 16).unwrap();
                std::alloc::dealloc(ptr as usize as *mut u8, layout);
            }
        },
    )?;

    Ok(())
}
