// Copyright (c) 2022  Mubelotix <Mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use std::mem::MaybeUninit;

/// # Safety
/// 
/// You must initialize the array after calling this function.
pub unsafe fn uninit_array() -> [u8; 32] {
    let array: [MaybeUninit<u8>; 32] = MaybeUninit::uninit().assume_init();
    std::mem::transmute(array)
}
