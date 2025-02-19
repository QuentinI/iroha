//! This crate provides utils for encoding/decoding objects between Iroha host and Wasm smart contracts.

#![allow(clippy::arithmetic_side_effects)]

pub use iroha_core_wasm_codec_derive::{wrap, wrap_trait_fn};
use parity_scale_codec::{DecodeAll, Encode};
use wasmtime::Trap;

/// [`usize`] of wasm
pub type WasmUsize = u32;

/// Decode object from the given `memory` at the given `offset` with the given `len`
///
/// # Warning
///
/// This method does not take ownership of the pointer.
///
/// # Errors
///
/// Fails with [`Trap`] if decoding fails.
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_from_memory<C: wasmtime::AsContext, T: DecodeAll>(
    memory: &wasmtime::Memory,
    context: &C,
    offset: WasmUsize,
    len: WasmUsize,
) -> Result<T, Trap> {
    // Accessing memory as a byte slice to avoid the use of unsafe
    let mem_range = offset as usize..(offset + len) as usize;
    let mut bytes = &memory.data(context)[mem_range];
    T::decode_all(&mut bytes).map_err(|error| Trap::new(error.to_string()))
}

/// Decode the object from a given pointer where first element is the size of the object
/// following it. This can be considered a custom encoding format.
///
/// # Warning
///
/// This method takes ownership of the given pointer.
///
/// # Errors
///
/// - Failed to decode object
/// - Failed to call `dealloc_fn`
#[allow(clippy::expect_used, clippy::unwrap_in_result)]
pub fn decode_with_length_prefix_from_memory<
    C: wasmtime::AsContextMut,
    T: DecodeAll + std::fmt::Debug,
>(
    memory: &wasmtime::Memory,
    dealloc_fn: &wasmtime::TypedFunc<(WasmUsize, WasmUsize), ()>,
    mut context: &mut C,
    offset: WasmUsize,
) -> Result<T, Trap> {
    const U32_TO_USIZE_ERROR_MES: &str = "`u32` should always fit in `usize`";

    let len_size_bytes: u32 = core::mem::size_of::<WasmUsize>()
        .try_into()
        .expect("`u32` size should always fit into `u32`");
    let len = u32::from_le_bytes(
        memory.data(&mut context)[offset as usize..(offset + len_size_bytes) as usize]
            .try_into()
            .expect("Prefix length size(bytes) incorrect"),
    );

    let bytes = &memory.data_mut(&mut context)[offset.try_into().expect(U32_TO_USIZE_ERROR_MES)
        ..(offset + len).try_into().expect(U32_TO_USIZE_ERROR_MES)];

    let obj =
        T::decode_all(&mut &bytes[len_size_bytes.try_into().expect(U32_TO_USIZE_ERROR_MES)..])
            .map_err(|err| Trap::new(format!("Failed to decode object: {err}")))?;

    dealloc_fn
        .call(&mut context, (offset, len))
        .map_err(|err| Trap::new(format!("Failed to call dealloc fn: {err}")))?; // TODO
    Ok(obj)
}

/// Encode `obj` to the given `memory` with the given `alloc_fn` and `context`.
///
/// Returns the offset of the encoded object.
///
/// # Errors
///
/// - If failed to call `alloc_fn`
/// - If failed to write into the `memory`
pub fn encode_into_memory<T: Encode>(
    obj: &T,
    memory: &wasmtime::Memory,
    alloc_fn: &wasmtime::TypedFunc<WasmUsize, WasmUsize>,
    mut context: impl wasmtime::AsContextMut,
) -> Result<WasmUsize, Trap> {
    let bytes = encode_with_length_prefix(obj);

    let len = bytes
        .len()
        .try_into()
        .expect("Checked in `encode_with_length_prefix`");

    let offset = alloc_fn.call(&mut context, len)?;
    let offset_usize = offset
        .try_into()
        .expect("`u32` should always fit in `usize`");

    memory
        .write(&mut context, offset_usize, &bytes)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(offset)
}

/// Encode the given object but also add it's length in front of it. This can be considered
/// a custom encoding format.
///
/// Usually, to retrieve the encoded object both pointer and the length of the allocation
/// are provided. However, due to the lack of support for multivalue return values in stable
/// `WebAssembly` it's not possible to return two values from a wasm function without some
/// shenanignas. In those cases, only one value is sent which is pointer to the allocation
/// with the first element being the length of the encoded object following it.
pub fn encode_with_length_prefix<T: Encode>(obj: &T) -> Vec<u8> {
    // Compile-time size check
    #[allow(clippy::let_unit_value)]
    let _ = SizeChecker::<T>::RESULT;

    let len_size_bytes = core::mem::size_of::<WasmUsize>();

    let mut r = Vec::with_capacity(len_size_bytes + obj.size_hint());

    // Reserve space for length
    r.resize(len_size_bytes, 0);
    obj.encode_to(&mut r);

    // Store length as byte array in front of encoding
    let len =
        &WasmUsize::try_from(r.len()).expect("Length is checked at compile time, this is a bug");
    r[..len_size_bytes].copy_from_slice(&len.to_le_bytes());

    r
}

/// Helper struct to check type size at compile time.
struct SizeChecker<T>(std::marker::PhantomData<T>);

impl<T> SizeChecker<T> {
    const RESULT: () = assert!(
        core::mem::size_of::<T>() < (u32::MAX as usize - core::mem::size_of::<WasmUsize>()),
        "Size doesn't fit in 2^32"
    );
}
