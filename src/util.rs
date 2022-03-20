use std::mem::MaybeUninit;

/// # Safety
/// 
/// You must initialize the array after calling this function.
pub unsafe fn uninit_array() -> [u8; 32] {
    let array: [MaybeUninit<u8>; 32] = MaybeUninit::uninit().assume_init();
    std::mem::transmute(array)
}
