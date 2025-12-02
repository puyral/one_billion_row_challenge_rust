use std::{
    hash::{BuildHasher, Hasher},
    simd::u8x4,
};

#[derive(Default)]
pub struct FasHaserBuilder;
pub struct FashHaser(uhash);

type uhash = u32;
static HALF_SIZE: u32 = std::mem::size_of::<uhash>() as u32 / 2;

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
        let len = bytes.len();
        if len <= 16 {
            if len >= 8 {
                let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                let hi = u64::from_le_bytes(bytes[len - 8..].try_into().unwrap());
                self.write_u64(lo);
                self.write_u64(hi);
            } else if len >= 4 {
                let lo = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
                let hi = u32::from_le_bytes(bytes[len - 4..].try_into().unwrap());
                self.0 ^= self.0.rotate_left(HALF_SIZE) ^ lo ^ hi
            } else if len > 0 {
                let lo = bytes[0] as u32;
                let mid = bytes[len / 2] as u32;
                let hi = bytes[len - 1] as u32;
                self.0 ^= lo ^ (mid << 8) ^ (hi << 16)
            }
            self.0.wrapping_add(3851351);
        } else {
            self.long_hash(bytes);
        }
    }

    fn write_length_prefix(&mut self, len: usize) {}

    fn write_u32(&mut self, i: u32) {
        self.0 ^= self.0.rotate_left(HALF_SIZE) ^ i
    }

    fn write_u64(&mut self, i: u64) {
        self.0 ^= self.0.rotate_left(HALF_SIZE) ^ (i as u32) ^ ((i >> 32) as u32)
    }
}

impl FashHaser {
    fn long_hash(&mut self, bytes: &[u8]) {
        let (chunks, _) = bytes.as_chunks();
        // for x in chunks {
        //     // self.0 ^= u8xh::from_array(*x);
        //     self.0 = uhash::wrapping_add(35435135, self.0.rotate_right(HALF_SIZE as u32)).wrapping_add(
        //         uhash::from_ne_bytes(*x));
        // }
        let res: uhash = chunks
            .iter()
            .map(|x| uhash::from_ne_bytes(*x))
            .reduce(|x, y| uhash::wrapping_add(54681627, x ^ y))
            .unwrap_or(0);
        self.0 ^= res;

        let rem = uhash::from_le_bytes(
            bytes[bytes.len() - std::mem::size_of::<uhash>()..]
                .try_into()
                .unwrap(),
        );
        self.0 ^= rem;
    }
}

pub fn to_usize_padded(slice: &[u8]) -> uhash {
    // // 1. Create a zeroed buffer on the stack (register width)
    // const SIZE: usize = std::mem::size_of::<uhash>();
    // debug_assert!(slice.len() <= SIZE);
    // let mut buf = [0u8; SIZE];

    // // 2. Determine how many bytes we can actually read
    // // let len = slice.len().min(SIZE);

    // // 3. Copy only the available bytes
    // // unsafe is used here for copy_nonoverlapping, which is slightly faster
    // // than slice::copy_from_slice because it skips some bounds checks.
    // unsafe {
    //     std::ptr::copy_nonoverlapping(slice.as_ptr(), buf.as_mut_ptr(), slice.len());
    // }

    let n = slice.len();
    let buf = ::std::array::from_fn(|i| slice[n.saturating_sub(i + 1)]);

    // 4. Convert to usize
    uhash::from_ne_bytes(buf)
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
            self.0 = (
                // u8xh::splat(MAGIC) *
                self.0.rotate_elements_right::<HALF_LENGTH>()
            ) ^ u8xh::from_array(*x);
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

/// I give up, my hashes have too many colisions
pub type MHasher = rustc_hash::FxBuildHasher;
// pub type MHasher = FasHaserBuilder;
// pub type MHasher = FasHaserBuilderSimd;
