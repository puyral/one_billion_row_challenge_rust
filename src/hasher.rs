use std::{
    hash::{BuildHasher, Hasher},
    simd::u8x4,
};

#[derive(Default)]
pub struct FasHaserBuilder;
pub struct FashHaser(uhash);

type uhash = usize;
static HALF_SIZE: usize = std::mem::size_of::<uhash>() / 2;

impl BuildHasher for FasHaserBuilder {
    type Hasher = FashHaser;

    fn build_hasher(&self) -> Self::Hasher {
        // FashHaser(u8xh::splat(MAGIC))
        FashHaser(5)
    }
}

impl Hasher for FashHaser {
    fn finish(&self) -> u64 {
        // u32::from_ne_bytes(*self.0.as_array()) as u64
        self.0 as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        let (chunks, remained) = bytes.as_chunks();
        for x in chunks {
            // self.0 ^= u8xh::from_array(*x);
            self.0 = uhash::wrapping_mul(49, self.0.rotate_right(HALF_SIZE as u32))
                ^ uhash::from_ne_bytes(*x);
        }
        // u32::from
        // self.0 ^= *remained.first().unwrap_or(&0) as u32
        self.0 = uhash::wrapping_mul(49, self.0.rotate_right(HALF_SIZE as u32))
            ^ to_usize_padded(remained)
    }
}

pub fn to_usize_padded(slice: &[u8]) -> uhash {
    // 1. Create a zeroed buffer on the stack (register width)
    const SIZE: usize = std::mem::size_of::<uhash>();
    debug_assert!(slice.len() <= SIZE);
    let mut buf = [0u8; SIZE];

    // 2. Determine how many bytes we can actually read
    // let len = slice.len().min(SIZE);

    // 3. Copy only the available bytes
    // unsafe is used here for copy_nonoverlapping, which is slightly faster
    // than slice::copy_from_slice because it skips some bounds checks.
    unsafe {
        std::ptr::copy_nonoverlapping(slice.as_ptr(), buf.as_mut_ptr(), slice.len());
    }

    // 4. Convert to usize
    usize::from_ne_bytes(buf)
}

#[derive(Default)]
pub struct FasHaserBuilderSimd;
pub struct FashHaserSimd(u8xh);
type u8xh = u8x4;
static HALF_LENGTH: usize = u8xh::LEN / 2;
static MAGIC: u8 = 13;

impl BuildHasher for FasHaserBuilderSimd {
    type Hasher = FashHaserSimd;

    fn build_hasher(&self) -> Self::Hasher {
        FashHaserSimd(u8xh::splat(MAGIC))
    }
}

impl Hasher for FashHaserSimd {
    fn finish(&self) -> u64 {
        u32::from_ne_bytes(*self.0.as_array()) as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        let (chunks, remained) = bytes.as_chunks();
        for x in chunks {
            self.0 = (u8xh::splat(MAGIC) * self.0.rotate_elements_right::<HALF_LENGTH>())
                ^ u8xh::from_array(*x);
        }
        self.0 ^= u8xh::load_or_default(remained);
    }

    fn write_length_prefix(&mut self, len: usize) {}

    fn write_usize(&mut self, i: usize) {
        let [x, y] = split_u64_unsafe(i as u64);
        self.0 = (u8xh::splat(MAGIC) * self.0.rotate_elements_right::<HALF_LENGTH>()) ^ x ^ y;
    }
}

fn split_u64_unsafe(value: u64) -> [u8x4; 2] {
    unsafe {
        // Transmute u64 directly into an array of two u8x4s
        // This relies on u8x4 having the same layout as [u8; 4]
        std::mem::transmute(value)
    }
}

// type MHasher = rustc_hash::FxBuildHasher;
// type MHasher = FasHaserBuilder;
pub type MHasher = FasHaserBuilderSimd;
