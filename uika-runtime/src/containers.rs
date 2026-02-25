// Container wrappers for UE TArray/TMap/TSet.
// These are lightweight handles that operate on container data living
// inside UObject memory. All element access goes through the ContainerApi
// FFI sub-table, which inspects FProperty to dispatch type-correct operations.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use uika_ffi::{FPropertyHandle, UObjectHandle, UikaErrorCode};

/// Maximum buffer size across all ContainerElement implementations.
/// Primitives use 1–8 bytes; String and OwnedStruct use 4096.
/// Used for stack-allocated FFI transport buffers to avoid heap allocation.
const MAX_ELEM_BUF: usize = 4096;

use crate::api::api;
use crate::error::{check_ffi, ffi_infallible, UikaError, UikaResult};
use crate::object_ref::UObjectRef;
use crate::struct_ref::UStructRef;
use crate::traits::{UeClass, UeStruct};

// ---------------------------------------------------------------------------
// ContainerElement trait
// ---------------------------------------------------------------------------

/// Trait for types that can be stored in UE containers.
///
/// # Safety
/// `BUF_SIZE` must match what the C++ side expects for this element type.
/// `read_from_buf` must correctly interpret the bytes written by C++'s
/// `ReadElement`, and `write_to_buf` must produce bytes that C++'s
/// `WriteElement` can interpret.
pub unsafe trait ContainerElement: Sized {
    /// Buffer size for FFI transport. Must be large enough for the C++ side
    /// to write the element value.
    const BUF_SIZE: u32;

    /// Whether this type can be bulk-copied as raw bytes (no per-element framing).
    /// True for fixed-size primitives (bool, integers, floats) and FName.
    const RAW_COPYABLE: bool = false;

    /// Interpret bytes from the C++ side into a Rust value.
    ///
    /// # Safety
    /// `buf` must point to at least `written` valid bytes produced by C++
    /// `ReadElement`.
    unsafe fn read_from_buf(buf: *const u8, written: u32) -> Self;

    /// Write this value into a buffer for C++ `WriteElement` to consume.
    /// Returns the number of bytes written.
    ///
    /// # Safety
    /// `buf` must point to at least `BUF_SIZE` writable bytes.
    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32;
}

// ---------------------------------------------------------------------------
// ContainerElement impls for primitives
// ---------------------------------------------------------------------------

macro_rules! impl_container_element_primitive {
    ($ty:ty) => {
        unsafe impl ContainerElement for $ty {
            const BUF_SIZE: u32 = std::mem::size_of::<$ty>() as u32;
            const RAW_COPYABLE: bool = true;

            #[inline]
            unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self { unsafe {
                (buf as *const $ty).read_unaligned()
            }}

            #[inline]
            unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
                (buf as *mut $ty).write_unaligned(*self);
                std::mem::size_of::<$ty>() as u32
            }}
        }
    };
}

impl_container_element_primitive!(bool);
impl_container_element_primitive!(i8);
impl_container_element_primitive!(u8);
impl_container_element_primitive!(i16);
impl_container_element_primitive!(u16);
impl_container_element_primitive!(i32);
impl_container_element_primitive!(u32);
impl_container_element_primitive!(i64);
impl_container_element_primitive!(u64);
impl_container_element_primitive!(f32);
impl_container_element_primitive!(f64);

// UObjectHandle: 8-byte pointer, raw memcpy in C++
unsafe impl ContainerElement for UObjectHandle {
    const BUF_SIZE: u32 = std::mem::size_of::<UObjectHandle>() as u32;

    #[inline]
    unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self { unsafe {
        (buf as *const UObjectHandle).read_unaligned()
    }}

    #[inline]
    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
        (buf as *mut UObjectHandle).write_unaligned(*self);
        std::mem::size_of::<UObjectHandle>() as u32
    }}
}

// FNameHandle: 8-byte uint64, raw memcpy in C++
unsafe impl ContainerElement for uika_ffi::FNameHandle {
    const BUF_SIZE: u32 = 8;
    const RAW_COPYABLE: bool = true;

    #[inline]
    unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self { unsafe {
        (buf as *const uika_ffi::FNameHandle).read_unaligned()
    }}

    #[inline]
    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
        (buf as *mut uika_ffi::FNameHandle).write_unaligned(*self);
        8
    }}
}

// UObjectRef<T>: delegates to UObjectHandle (8-byte pointer)
unsafe impl<T: UeClass> ContainerElement for UObjectRef<T> {
    const BUF_SIZE: u32 = std::mem::size_of::<UObjectHandle>() as u32;

    #[inline]
    unsafe fn read_from_buf(buf: *const u8, _written: u32) -> Self { unsafe {
        let handle = (buf as *const UObjectHandle).read_unaligned();
        UObjectRef::from_raw(handle)
    }}

    #[inline]
    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
        (buf as *mut UObjectHandle).write_unaligned(self.raw());
        std::mem::size_of::<UObjectHandle>() as u32
    }}
}

// String: C++ uses [u32 len][utf8 bytes] format
unsafe impl ContainerElement for String {
    // Max buffer: 4 bytes length prefix + up to 4092 bytes of UTF-8 data
    const BUF_SIZE: u32 = 4096;

    unsafe fn read_from_buf(buf: *const u8, written: u32) -> Self { unsafe {
        if written < 4 {
            return String::new();
        }
        let len = (buf as *const u32).read_unaligned() as usize;
        let data_len = (written as usize).saturating_sub(4).min(len);
        let slice = std::slice::from_raw_parts(buf.add(4), data_len);
        String::from_utf8_lossy(slice).into_owned()
    }}

    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
        let bytes = self.as_bytes();
        let len = bytes.len() as u32;
        (buf as *mut u32).write_unaligned(len);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.add(4), bytes.len());
        4 + len
    }}
}

// ---------------------------------------------------------------------------
// UeArray<T>
// ---------------------------------------------------------------------------

/// A view into a UE `TArray<T>` property on a UObject.
///
/// This is a lightweight `Copy` handle — it does not own the data.
/// All operations go through FFI calls to the C++ container API.
#[derive(Clone, Copy)]
pub struct UeArray<T: ContainerElement> {
    owner: UObjectHandle,
    prop: FPropertyHandle,
    _marker: PhantomData<T>,
}

impl<T: ContainerElement> UeArray<T> {
    /// Create a new array view from an owner object handle and a property handle.
    #[inline]
    pub fn new(owner: UObjectHandle, prop: FPropertyHandle) -> Self {
        UeArray {
            owner,
            prop,
            _marker: PhantomData,
        }
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> UikaResult<usize> {
        let n = unsafe { ((*api().container).array_len)(self.owner, self.prop) };
        if n < 0 {
            return Err(UikaError::ObjectDestroyed);
        }
        Ok(n as usize)
    }

    /// Returns true if the array is empty.
    pub fn is_empty(&self) -> UikaResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Get the element at `index`.
    pub fn get(&self, index: usize) -> UikaResult<T> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        let mut written: u32 = 0;
        check_ffi(unsafe {
            ((*api().container).array_get)(
                self.owner,
                self.prop,
                index as i32,
                buf.as_mut_ptr(),
                T::BUF_SIZE,
                &mut written,
            )
        })?;
        Ok(unsafe { T::read_from_buf(buf.as_ptr(), written) })
    }

    /// Set the element at `index`.
    pub fn set(&self, index: usize, val: &T) -> UikaResult<()> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        // SAFETY: buf is freshly allocated with BUF_SIZE bytes.
        let written = unsafe { val.write_to_buf(buf.as_mut_ptr()) };
        check_ffi(unsafe {
            ((*api().container).array_set)(
                self.owner,
                self.prop,
                index as i32,
                buf.as_ptr(),
                written,
            )
        })
    }

    /// Append an element to the end of the array.
    pub fn push(&self, val: &T) -> UikaResult<()> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        // SAFETY: buf is freshly allocated with BUF_SIZE bytes.
        let written = unsafe { val.write_to_buf(buf.as_mut_ptr()) };
        check_ffi(unsafe {
            ((*api().container).array_add)(self.owner, self.prop, buf.as_ptr(), written)
        })
    }

    /// Remove the element at `index`, shifting subsequent elements down.
    pub fn remove(&self, index: usize) -> UikaResult<()> {
        check_ffi(unsafe {
            ((*api().container).array_remove)(self.owner, self.prop, index as i32)
        })
    }

    /// Remove all elements from the array.
    pub fn clear(&self) -> UikaResult<()> {
        check_ffi(unsafe { ((*api().container).array_clear)(self.owner, self.prop) })
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> UeArrayIter<'_, T> {
        let len = self.len().unwrap_or(0);
        UeArrayIter {
            array: self,
            index: 0,
            len,
        }
    }
}

impl<'a, T: ContainerElement> IntoIterator for &'a UeArray<T> {
    type Item = UikaResult<T>;
    type IntoIter = UeArrayIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over `UeArray<T>` elements.
pub struct UeArrayIter<'a, T: ContainerElement> {
    array: &'a UeArray<T>,
    index: usize,
    len: usize,
}

impl<T: ContainerElement> Iterator for UeArrayIter<'_, T> {
    type Item = UikaResult<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let result = self.array.get(self.index);
        self.index += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<T: ContainerElement> ExactSizeIterator for UeArrayIter<'_, T> {}

// ---------------------------------------------------------------------------
// Bulk copy helper
// ---------------------------------------------------------------------------

/// Perform a bulk copy FFI call with automatic retry on BufferTooSmall.
fn bulk_copy_with_retry(
    estimate: usize,
    call: impl Fn(*mut u8, u32, *mut u32, *mut i32) -> UikaErrorCode,
) -> UikaResult<(Vec<u8>, i32)> {
    let mut buf = vec![0u8; estimate.max(64)];
    let mut written: u32 = 0;
    let mut count: i32 = 0;
    let code = call(buf.as_mut_ptr(), buf.len() as u32, &mut written, &mut count);
    if code == UikaErrorCode::BufferTooSmall {
        // Retry with the size hint from C++
        let needed = (written as usize).max(buf.len() * 2);
        buf.resize(needed, 0);
        check_ffi(call(
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut written,
            &mut count,
        ))?;
    } else {
        check_ffi(code)?;
    }
    buf.truncate(written as usize);
    Ok((buf, count))
}

// ---------------------------------------------------------------------------
// Bulk iterators
// ---------------------------------------------------------------------------

/// Iterator that owns a pre-fetched buffer from a single bulk FFI call.
/// Yields elements without further FFI calls.
///
/// Supports two modes:
/// - **Framed** (`raw_elem_size == 0`): `[u32 written][data]` per element
/// - **Raw** (`raw_elem_size > 0`): contiguous fixed-size elements, no framing
pub struct BulkArrayIter<T: ContainerElement> {
    buf: Vec<u8>,
    count: usize,
    index: usize,
    offset: usize,
    raw_elem_size: usize, // 0 = framed, >0 = raw stride
    _marker: PhantomData<T>,
}

impl<T: ContainerElement> Iterator for BulkArrayIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.index >= self.count {
            return None;
        }
        if self.raw_elem_size > 0 {
            // Raw mode: elements are contiguous with fixed stride
            let elem = unsafe {
                T::read_from_buf(
                    self.buf.as_ptr().add(self.offset),
                    self.raw_elem_size as u32,
                )
            };
            self.offset += self.raw_elem_size;
            self.index += 1;
            Some(elem)
        } else {
            // Framed mode: [u32 written][data] per element
            let written = u32::from_ne_bytes(
                self.buf[self.offset..self.offset + 4].try_into().unwrap(),
            );
            self.offset += 4;
            let elem =
                unsafe { T::read_from_buf(self.buf.as_ptr().add(self.offset), written) };
            self.offset += written as usize;
            self.index += 1;
            Some(elem)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count - self.index;
        (r, Some(r))
    }
}

impl<T: ContainerElement> ExactSizeIterator for BulkArrayIter<T> {}

/// Bulk iterator over `TMap<K,V>` key-value pairs. Single FFI call.
pub struct BulkMapIter<K: ContainerElement, V: ContainerElement> {
    buf: Vec<u8>,
    count: usize,
    index: usize,
    offset: usize,
    _marker: PhantomData<(K, V)>,
}

impl<K: ContainerElement, V: ContainerElement> Iterator for BulkMapIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<(K, V)> {
        if self.index >= self.count {
            return None;
        }
        // Key: [u32 written][data]
        let key_written = u32::from_ne_bytes(
            self.buf[self.offset..self.offset + 4].try_into().unwrap(),
        );
        self.offset += 4;
        let key =
            unsafe { K::read_from_buf(self.buf.as_ptr().add(self.offset), key_written) };
        self.offset += key_written as usize;

        // Value: [u32 written][data]
        let val_written = u32::from_ne_bytes(
            self.buf[self.offset..self.offset + 4].try_into().unwrap(),
        );
        self.offset += 4;
        let val =
            unsafe { V::read_from_buf(self.buf.as_ptr().add(self.offset), val_written) };
        self.offset += val_written as usize;

        self.index += 1;
        Some((key, val))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count - self.index;
        (r, Some(r))
    }
}

impl<K: ContainerElement, V: ContainerElement> ExactSizeIterator for BulkMapIter<K, V> {}

/// Bulk iterator over `TSet<T>` elements. Single FFI call.
pub struct BulkSetIter<T: ContainerElement> {
    buf: Vec<u8>,
    count: usize,
    index: usize,
    offset: usize,
    _marker: PhantomData<T>,
}

impl<T: ContainerElement> Iterator for BulkSetIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.index >= self.count {
            return None;
        }
        let written = u32::from_ne_bytes(
            self.buf[self.offset..self.offset + 4].try_into().unwrap(),
        );
        self.offset += 4;
        let elem = unsafe { T::read_from_buf(self.buf.as_ptr().add(self.offset), written) };
        self.offset += written as usize;
        self.index += 1;
        Some(elem)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.count - self.index;
        (r, Some(r))
    }
}

impl<T: ContainerElement> ExactSizeIterator for BulkSetIter<T> {}

// ---------------------------------------------------------------------------
// UeArray bulk methods
// ---------------------------------------------------------------------------

impl<T: ContainerElement> UeArray<T> {
    /// Bulk-copy all elements to a `Vec<T>` in a single FFI call.
    pub fn to_vec(&self) -> UikaResult<Vec<T>> {
        let len = self.len()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let owner = self.owner;
        let prop = self.prop;
        // Raw-copyable types have no per-element framing overhead
        let estimate = if T::RAW_COPYABLE {
            len * T::BUF_SIZE as usize
        } else {
            len * (T::BUF_SIZE as usize + 4)
        };
        let (buf, count) = bulk_copy_with_retry(estimate, |out, size, written, cnt| unsafe {
            ((*api().container).array_copy_all)(owner, prop, out, size, written, cnt)
        })?;
        // Negative count = raw format from C++
        let (actual_count, raw_elem_size) = if count < 0 {
            ((-count) as usize, T::BUF_SIZE as usize)
        } else {
            (count as usize, 0)
        };
        Ok(BulkArrayIter::<T> {
            buf,
            count: actual_count,
            index: 0,
            offset: 0,
            raw_elem_size,
            _marker: PhantomData,
        }
        .collect())
    }

    /// Bulk-fetch all elements as a lazy iterator (single FFI call).
    pub fn bulk_iter(&self) -> UikaResult<BulkArrayIter<T>> {
        let len = self.len()?;
        if len == 0 {
            return Ok(BulkArrayIter {
                buf: Vec::new(),
                count: 0,
                index: 0,
                offset: 0,
                raw_elem_size: 0,
                _marker: PhantomData,
            });
        }
        let owner = self.owner;
        let prop = self.prop;
        let estimate = if T::RAW_COPYABLE {
            len * T::BUF_SIZE as usize
        } else {
            len * (T::BUF_SIZE as usize + 4)
        };
        let (buf, count) = bulk_copy_with_retry(estimate, |out, size, written, cnt| unsafe {
            ((*api().container).array_copy_all)(owner, prop, out, size, written, cnt)
        })?;
        let (actual_count, raw_elem_size) = if count < 0 {
            ((-count) as usize, T::BUF_SIZE as usize)
        } else {
            (count as usize, 0)
        };
        Ok(BulkArrayIter {
            buf,
            count: actual_count,
            index: 0,
            offset: 0,
            raw_elem_size,
            _marker: PhantomData,
        })
    }

    /// Replace all array elements from a slice in a single FFI call.
    pub fn set_all(&self, items: &[T]) -> UikaResult<()> {
        if items.is_empty() {
            return self.clear();
        }
        if T::RAW_COPYABLE {
            // Raw format: contiguous elements, no per-element framing
            let elem_size = T::BUF_SIZE as usize;
            let mut buf = vec![0u8; items.len() * elem_size];
            for (i, item) in items.iter().enumerate() {
                unsafe { item.write_to_buf(buf.as_mut_ptr().add(i * elem_size)); }
            }
            // Negative count signals raw format to C++
            check_ffi(unsafe {
                ((*api().container).array_set_all)(
                    self.owner,
                    self.prop,
                    buf.as_ptr(),
                    buf.len() as u32,
                    -(items.len() as i32),
                )
            })
        } else {
            // Framed format: [u32 written][data] per element
            let mut buf = Vec::with_capacity(items.len() * (T::BUF_SIZE as usize + 4));
            let mut elem_buf = [0u8; MAX_ELEM_BUF];
            for item in items {
                let written = unsafe { item.write_to_buf(elem_buf.as_mut_ptr()) };
                buf.extend_from_slice(&written.to_ne_bytes());
                buf.extend_from_slice(&elem_buf[..written as usize]);
            }
            check_ffi(unsafe {
                ((*api().container).array_set_all)(
                    self.owner,
                    self.prop,
                    buf.as_ptr(),
                    buf.len() as u32,
                    items.len() as i32,
                )
            })
        }
    }
}

// ---------------------------------------------------------------------------
// UeMap<K, V>
// ---------------------------------------------------------------------------

/// A view into a UE `TMap<K, V>` property on a UObject.
#[derive(Clone, Copy)]
pub struct UeMap<K: ContainerElement, V: ContainerElement> {
    owner: UObjectHandle,
    prop: FPropertyHandle,
    _marker: PhantomData<(K, V)>,
}

impl<K: ContainerElement, V: ContainerElement> UeMap<K, V> {
    #[inline]
    pub fn new(owner: UObjectHandle, prop: FPropertyHandle) -> Self {
        UeMap {
            owner,
            prop,
            _marker: PhantomData,
        }
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> UikaResult<usize> {
        let n = unsafe { ((*api().container).map_len)(self.owner, self.prop) };
        if n < 0 {
            return Err(UikaError::ObjectDestroyed);
        }
        Ok(n as usize)
    }

    pub fn is_empty(&self) -> UikaResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Look up a value by key. Returns `Err(PropertyNotFound)` if the key
    /// is not in the map.
    pub fn find(&self, key: &K) -> UikaResult<V> {
        let mut key_buf = [0u8; MAX_ELEM_BUF];
        let key_written = unsafe { key.write_to_buf(key_buf.as_mut_ptr()) };
        let mut val_buf = [0u8; MAX_ELEM_BUF];
        let mut val_written: u32 = 0;

        check_ffi(unsafe {
            ((*api().container).map_find)(
                self.owner,
                self.prop,
                key_buf.as_ptr(),
                key_written,
                val_buf.as_mut_ptr(),
                V::BUF_SIZE,
                &mut val_written,
            )
        })?;
        Ok(unsafe { V::read_from_buf(val_buf.as_ptr(), val_written) })
    }

    /// Insert or replace a key-value pair.
    pub fn add(&self, key: &K, val: &V) -> UikaResult<()> {
        let mut key_buf = [0u8; MAX_ELEM_BUF];
        let key_written = unsafe { key.write_to_buf(key_buf.as_mut_ptr()) };
        let mut val_buf = [0u8; MAX_ELEM_BUF];
        let val_written = unsafe { val.write_to_buf(val_buf.as_mut_ptr()) };

        check_ffi(unsafe {
            ((*api().container).map_add)(
                self.owner,
                self.prop,
                key_buf.as_ptr(),
                key_written,
                val_buf.as_ptr(),
                val_written,
            )
        })
    }

    /// Remove a key from the map.
    pub fn remove(&self, key: &K) -> UikaResult<()> {
        let mut key_buf = [0u8; MAX_ELEM_BUF];
        let key_written = unsafe { key.write_to_buf(key_buf.as_mut_ptr()) };

        check_ffi(unsafe {
            ((*api().container).map_remove)(
                self.owner,
                self.prop,
                key_buf.as_ptr(),
                key_written,
            )
        })
    }

    /// Remove all key-value pairs.
    pub fn clear(&self) -> UikaResult<()> {
        check_ffi(unsafe { ((*api().container).map_clear)(self.owner, self.prop) })
    }

    /// Get the key-value pair at logical index (for iteration).
    pub fn get_pair(&self, logical_index: usize) -> UikaResult<(K, V)> {
        let mut key_buf = [0u8; MAX_ELEM_BUF];
        let mut key_written: u32 = 0;
        let mut val_buf = [0u8; MAX_ELEM_BUF];
        let mut val_written: u32 = 0;

        check_ffi(unsafe {
            ((*api().container).map_get_pair)(
                self.owner,
                self.prop,
                logical_index as i32,
                key_buf.as_mut_ptr(),
                K::BUF_SIZE,
                &mut key_written,
                val_buf.as_mut_ptr(),
                V::BUF_SIZE,
                &mut val_written,
            )
        })?;
        Ok(unsafe {
            (
                K::read_from_buf(key_buf.as_ptr(), key_written),
                V::read_from_buf(val_buf.as_ptr(), val_written),
            )
        })
    }

    /// Returns an iterator over key-value pairs.
    pub fn iter(&self) -> UeMapIter<'_, K, V> {
        let len = self.len().unwrap_or(0);
        UeMapIter {
            map: self,
            index: 0,
            len,
        }
    }
}

impl<'a, K: ContainerElement, V: ContainerElement> IntoIterator for &'a UeMap<K, V> {
    type Item = UikaResult<(K, V)>;
    type IntoIter = UeMapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over `UeMap<K, V>` key-value pairs.
pub struct UeMapIter<'a, K: ContainerElement, V: ContainerElement> {
    map: &'a UeMap<K, V>,
    index: usize,
    len: usize,
}

impl<K: ContainerElement, V: ContainerElement> Iterator for UeMapIter<'_, K, V> {
    type Item = UikaResult<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let result = self.map.get_pair(self.index);
        self.index += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<K: ContainerElement, V: ContainerElement> ExactSizeIterator for UeMapIter<'_, K, V> {}

// ---------------------------------------------------------------------------
// UeMap bulk methods
// ---------------------------------------------------------------------------

impl<K: ContainerElement, V: ContainerElement> UeMap<K, V> {
    /// Bulk-fetch all key-value pairs as a lazy iterator (single FFI call).
    pub fn bulk_iter(&self) -> UikaResult<BulkMapIter<K, V>> {
        let len = self.len()?;
        if len == 0 {
            return Ok(BulkMapIter {
                buf: Vec::new(),
                count: 0,
                index: 0,
                offset: 0,
                _marker: PhantomData,
            });
        }
        let owner = self.owner;
        let prop = self.prop;
        let estimate = len * (K::BUF_SIZE as usize + V::BUF_SIZE as usize + 8);
        let (buf, count) = bulk_copy_with_retry(estimate, |out, size, written, cnt| unsafe {
            ((*api().container).map_copy_all)(owner, prop, out, size, written, cnt)
        })?;
        Ok(BulkMapIter {
            buf,
            count: count as usize,
            index: 0,
            offset: 0,
            _marker: PhantomData,
        })
    }
}

impl<K: ContainerElement + Hash + Eq, V: ContainerElement> UeMap<K, V> {
    /// Bulk-copy all key-value pairs to a `HashMap` in a single FFI call.
    pub fn to_hash_map(&self) -> UikaResult<HashMap<K, V>> {
        let iter = self.bulk_iter()?;
        let count = iter.count;
        let mut map = HashMap::with_capacity(count);
        for (k, v) in iter {
            map.insert(k, v);
        }
        Ok(map)
    }
}

// ---------------------------------------------------------------------------
// UeSet<T>
// ---------------------------------------------------------------------------

/// A view into a UE `TSet<T>` property on a UObject.
#[derive(Clone, Copy)]
pub struct UeSet<T: ContainerElement> {
    owner: UObjectHandle,
    prop: FPropertyHandle,
    _marker: PhantomData<T>,
}

impl<T: ContainerElement> UeSet<T> {
    #[inline]
    pub fn new(owner: UObjectHandle, prop: FPropertyHandle) -> Self {
        UeSet {
            owner,
            prop,
            _marker: PhantomData,
        }
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> UikaResult<usize> {
        let n = unsafe { ((*api().container).set_len)(self.owner, self.prop) };
        if n < 0 {
            return Err(UikaError::ObjectDestroyed);
        }
        Ok(n as usize)
    }

    pub fn is_empty(&self) -> UikaResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Check if the set contains an element.
    pub fn contains(&self, val: &T) -> UikaResult<bool> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        let written = unsafe { val.write_to_buf(buf.as_mut_ptr()) };
        Ok(unsafe {
            ((*api().container).set_contains)(self.owner, self.prop, buf.as_ptr(), written)
        })
    }

    /// Add an element to the set.
    pub fn add(&self, val: &T) -> UikaResult<()> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        let written = unsafe { val.write_to_buf(buf.as_mut_ptr()) };
        check_ffi(unsafe {
            ((*api().container).set_add)(self.owner, self.prop, buf.as_ptr(), written)
        })
    }

    /// Remove an element from the set.
    pub fn remove(&self, val: &T) -> UikaResult<()> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        let written = unsafe { val.write_to_buf(buf.as_mut_ptr()) };
        check_ffi(unsafe {
            ((*api().container).set_remove)(self.owner, self.prop, buf.as_ptr(), written)
        })
    }

    /// Remove all elements from the set.
    pub fn clear(&self) -> UikaResult<()> {
        check_ffi(unsafe { ((*api().container).set_clear)(self.owner, self.prop) })
    }

    /// Get the element at logical index (for iteration).
    pub fn get_element(&self, logical_index: usize) -> UikaResult<T> {
        let mut buf = [0u8; MAX_ELEM_BUF];
        let mut written: u32 = 0;
        check_ffi(unsafe {
            ((*api().container).set_get_element)(
                self.owner,
                self.prop,
                logical_index as i32,
                buf.as_mut_ptr(),
                T::BUF_SIZE,
                &mut written,
            )
        })?;
        Ok(unsafe { T::read_from_buf(buf.as_ptr(), written) })
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> UeSetIter<'_, T> {
        let len = self.len().unwrap_or(0);
        UeSetIter {
            set: self,
            index: 0,
            len,
        }
    }
}

impl<'a, T: ContainerElement> IntoIterator for &'a UeSet<T> {
    type Item = UikaResult<T>;
    type IntoIter = UeSetIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over `UeSet<T>` elements.
pub struct UeSetIter<'a, T: ContainerElement> {
    set: &'a UeSet<T>,
    index: usize,
    len: usize,
}

impl<T: ContainerElement> Iterator for UeSetIter<'_, T> {
    type Item = UikaResult<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let result = self.set.get_element(self.index);
        self.index += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<T: ContainerElement> ExactSizeIterator for UeSetIter<'_, T> {}

// ---------------------------------------------------------------------------
// UeSet bulk methods
// ---------------------------------------------------------------------------

impl<T: ContainerElement> UeSet<T> {
    /// Bulk-fetch all elements as a lazy iterator (single FFI call).
    pub fn bulk_iter(&self) -> UikaResult<BulkSetIter<T>> {
        let len = self.len()?;
        if len == 0 {
            return Ok(BulkSetIter {
                buf: Vec::new(),
                count: 0,
                index: 0,
                offset: 0,
                _marker: PhantomData,
            });
        }
        let owner = self.owner;
        let prop = self.prop;
        let estimate = len * (T::BUF_SIZE as usize + 4);
        let (buf, count) = bulk_copy_with_retry(estimate, |out, size, written, cnt| unsafe {
            ((*api().container).set_copy_all)(owner, prop, out, size, written, cnt)
        })?;
        Ok(BulkSetIter {
            buf,
            count: count as usize,
            index: 0,
            offset: 0,
            _marker: PhantomData,
        })
    }
}

impl<T: ContainerElement + Hash + Eq> UeSet<T> {
    /// Bulk-copy all elements to a `HashSet` in a single FFI call.
    pub fn to_hash_set(&self) -> UikaResult<HashSet<T>> {
        let iter = self.bulk_iter()?;
        let count = iter.count;
        let mut set = HashSet::with_capacity(count);
        for elem in iter {
            set.insert(elem);
        }
        Ok(set)
    }
}

// ---------------------------------------------------------------------------
// OwnedStruct<T>: owned copy of struct data from a container
// ---------------------------------------------------------------------------

/// An owned copy of UE struct data retrieved from a container.
///
/// Since UE structs are opaque (their layout is managed by C++), this type
/// holds the raw bytes copied from the container. Use [`as_ref`](Self::as_ref)
/// to get a `UStructRef<T>` for property access.
pub struct OwnedStruct<T: UeStruct> {
    data: Vec<u8>,
    needs_destroy: bool,
    _marker: PhantomData<T>,
}

impl<T: UeStruct> OwnedStruct<T> {
    /// Allocate a new struct initialized via C++ default constructor.
    ///
    /// Uses the UE reflection system to determine the struct's size,
    /// allocates a zero-filled buffer, then calls `UScriptStruct::InitializeStruct`
    /// to properly construct non-trivial members (TArray, FString, etc.).
    /// The struct is destroyed via `UScriptStruct::DestroyStruct` on drop.
    pub fn new() -> Self {
        let ustruct = T::static_struct();
        let size = unsafe { ((*api().reflection).get_struct_size)(ustruct) };
        debug_assert!(size > 0, "get_struct_size returned 0 for {}", std::any::type_name::<T>());
        let mut data = vec![0u8; size as usize];
        ffi_infallible(unsafe {
            ((*api().reflection).initialize_struct)(ustruct, data.as_mut_ptr())
        });
        OwnedStruct {
            data,
            needs_destroy: true,
            _marker: PhantomData,
        }
    }

    /// Create from raw bytes (e.g., copied from a container element).
    ///
    /// The data is assumed to already be initialized by C++ — no destructor
    /// will be called on drop.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        OwnedStruct {
            data,
            needs_destroy: false,
            _marker: PhantomData,
        }
    }

    /// Get a `UStructRef<T>` for property access on this struct data.
    pub fn as_ref(&self) -> UStructRef<T> {
        unsafe { UStructRef::from_raw(self.data.as_ptr() as *mut u8) }
    }

    /// Get the raw bytes of the struct data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl<T: UeStruct> Clone for OwnedStruct<T> {
    fn clone(&self) -> Self {
        OwnedStruct {
            data: self.data.clone(),
            needs_destroy: false,
            _marker: PhantomData,
        }
    }
}

impl<T: UeStruct> Drop for OwnedStruct<T> {
    fn drop(&mut self) {
        if self.needs_destroy {
            unsafe {
                ((*api().reflection).destroy_struct)(
                    T::static_struct(),
                    self.data.as_mut_ptr(),
                );
            }
        }
    }
}

impl<T: UeStruct> std::fmt::Debug for OwnedStruct<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedStruct")
            .field("size", &self.data.len())
            .finish()
    }
}

// ContainerElement for OwnedStruct<T>: uses a fixed 4096-byte buffer.
// The C++ side copies struct data via CopyScriptStruct; we store the
// exact number of bytes written.
unsafe impl<T: UeStruct> ContainerElement for OwnedStruct<T> {
    const BUF_SIZE: u32 = 4096;

    unsafe fn read_from_buf(buf: *const u8, written: u32) -> Self { unsafe {
        let mut data = vec![0u8; written as usize];
        std::ptr::copy_nonoverlapping(buf, data.as_mut_ptr(), written as usize);
        OwnedStruct {
            data,
            needs_destroy: false,
            _marker: PhantomData,
        }
    }}

    unsafe fn write_to_buf(&self, buf: *mut u8) -> u32 { unsafe {
        let len = self.data.len();
        std::ptr::copy_nonoverlapping(self.data.as_ptr(), buf, len);
        len as u32
    }}
}
