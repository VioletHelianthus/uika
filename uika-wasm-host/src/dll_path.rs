//! Find `game.wasm` next to the current DLL.

use std::path::PathBuf;

/// Find `game.wasm` next to the current DLL.
///
/// Uses `GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, ...)` to locate
/// the HMODULE of this DLL, then replaces the filename with `game.wasm`.
pub fn find_wasm_file() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::MAX_PATH;
        use windows_sys::Win32::System::LibraryLoader::{
            GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        };

        unsafe {
            let mut hmodule = std::ptr::null_mut();
            let anchor = find_wasm_file as *const u8 as *const u16;
            let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
            if GetModuleHandleExW(flags, anchor, &mut hmodule) == 0 {
                return None;
            }

            let mut buf = [0u16; MAX_PATH as usize];
            let len = GetModuleFileNameW(hmodule, buf.as_mut_ptr(), buf.len() as u32);
            if len == 0 || len as usize >= buf.len() {
                return None;
            }

            let dll_path = String::from_utf16_lossy(&buf[..len as usize]);
            let mut path = PathBuf::from(dll_path);
            path.set_file_name("game.wasm");
            Some(path)
        }
    }

    #[cfg(not(windows))]
    {
        None
    }
}
