// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub enum Segment<T: Hash + std::fmt::Debug> {
    Item(T),
    SegmentedArray(SegmentedArray<T>),
    Unknown(Box<[u8; 32]>),
}

/// An array data type whose items can be removed without affecting its hash.
/// Note: order of items must not change.
#[derive(Debug, Clone)]
pub struct SegmentedArray<T: Hash + std::fmt::Debug> {
    pub segments: Vec<Segment<T>>,
}

impl<T: Hash + Clone + std::fmt::Debug> SegmentedArray<T> {
    pub fn remove_segment(&mut self, i: usize) {
        self.segments[i] = Segment::Unknown(self.segments[i].hash());
    }
}

impl<T: Hash + Clone + std::fmt::Debug> Segment<T> {
    fn items(self) -> Vec<T> {
        match self {
            Segment::Item(item) => vec![item],
            Segment::SegmentedArray(segmented_array) => segmented_array.items(),
            Segment::Unknown(_) => vec![],
        }
    }
}

impl<T: Hash + Clone + std::fmt::Debug> SegmentedArray<T> {
    fn items(self) -> Vec<T> {
        let mut items = Vec::new();
        for segment in self.segments {
            items.extend(segment.items());
        }
        items
    }
}

impl<T: Hash + Clone + std::fmt::Debug> From<SegmentedArray<T>> for Vec<T> {
    fn from(val: SegmentedArray<T>) -> Self {
        val.items()
    }
}

impl<T: Hash + Clone + std::fmt::Debug> From<Vec<T>> for SegmentedArray<T> {
    fn from(items: Vec<T>) -> Self {
        let mut segments: Vec<Segment<T>> = Vec::new();
        for item in items {
            segments.push(Segment::Item(item));
        }

        loop {
            if segments.len() <= 16 {
                return SegmentedArray { segments };
            }

            let mut segments_iter = segments.into_iter();
            let mut new_segments: Vec<Segment<T>> = Vec::new();
            
            while let Some(segment) = segments_iter.next() {
                let mut new_segmented_array = Vec::with_capacity(16);
                new_segmented_array.push(segment);
                for _ in 0..15 { // It's 15 and not 16 because we already have one item
                    match segments_iter.next() {
                        Some(segment) => new_segmented_array.push(segment),
                        None => break,
                    }
                }
                new_segments.push(Segment::SegmentedArray(SegmentedArray {
                    segments: new_segmented_array,
                }));
            }
            segments = new_segments;
        }
    }
}

impl<T: Hash + std::fmt::Debug> Hash for Segment<T> {
    fn hash(&self) -> Box<[u8; 32]> {
        match self {
            Segment::Item(item) => item.hash(),
            Segment::SegmentedArray(segmented_array) => segmented_array.hash(),
            Segment::Unknown(hash) => hash.clone(),
        }
    }
}

impl<T: Hash + std::fmt::Debug> Hash for SegmentedArray<T> {
    fn hash(&self) -> Box<[u8; 32]> {
        let mut hasher = Sha256::new();
        for segment in self.segments.iter() {
            hasher.update(segment.hash().as_slice());
        }

        let result = hasher.finalize();
        let mut hash: Box<[u8; 32]> = Box::new(unsafe { uninit_array() });
        hash.copy_from_slice(&result);
        
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_into_vec() {
        let mut array: Vec<u16> = Vec::new();
        for i in 0..49215 {
            array.push(i);
        }

        let seg_array = SegmentedArray::from(array.clone());
        let array2 = Vec::from(seg_array);

        assert_eq!(array, array2);
    }

    #[test]
    fn hash() {
        let mut array: Vec<u16> = Vec::new();
        for i in 0..51215 {
            array.push(i);
        }

        let mut seg_array = SegmentedArray::from(array.clone());
        let seg_array2 = seg_array.clone();

        seg_array.remove_segment(7);
        if let Some(Segment::SegmentedArray(array)) = seg_array.segments.get_mut(6) {
            array.remove_segment(5);
        }

        assert_eq!(seg_array.hash(), seg_array2.hash());
    }

    #[test]
    fn segment_sizes() {
        fn check_segment_size<T: Hash + std::fmt::Debug>(segment: &Segment<T>) {
            if let Segment::SegmentedArray(array) = segment {
                assert!(array.segments.len() <= 16);
            }
        }

        let mut array: Vec<u16> = Vec::new();
        for i in 0..51215 {
            array.push(i);
        }

        let mut seg_array = SegmentedArray::from(array.clone());

        for segment in seg_array.segments.iter_mut() {
            check_segment_size(segment);
        }
    }
}
