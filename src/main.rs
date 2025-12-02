#![feature(portable_simd)]
#![feature(hasher_prefixfree_extras)]
#![allow(unused)]
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fmt::Display,
    fs::{self, File},
    hash::{BuildHasher, Hash, Hasher},
    io::{BufRead, BufReader},
    simd::{prelude::SimdPartialEq, u8x4, u8x8, u8x16, u8x32, u8x64},
    str::Chars,
};

use memchr::Memchr2;
use memmap2::{Mmap, MmapOptions};
use rustc_hash::FxHashMap;
use smallvec::{SmallVec, ToSmallVec};

#[allow(nonstandard_style)]
type fsize = i16;

struct Stat {
    min: fsize,
    max: fsize,
    sum: i64,
    count: u32,
}

impl Default for Stat {
    fn default() -> Self {
        Self {
            min: fsize::MAX,
            max: fsize::MIN,
            sum: 0,
            count: 0,
        }
    }
}

#[derive(Default)]
struct FasHaserBuilder;
struct FashHaser(uhash);

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
struct FasHaserBuilderSimd;
struct FashHaserSimd(u8xh);
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

// type ArrayType = SmallVec<[u8; 16]>;
type ArrayType = Vec<u8>;

// type MHasher = rustc_hash::FxBuildHasher;
// type MHasher = FasHaserBuilder;
type MHasher = FasHaserBuilderSimd;

fn main() {
    let f = File::open("measurements.txt").unwrap();
    let f = unsafe { Mmap::map(&f).unwrap() };
    f.advise(memmap2::Advice::Sequential).unwrap();

    let mut stats = HashMap::with_capacity_and_hasher(10000, MHasher::default()); //  ahash::RandomState::new());
    let iter = Finder::new(&f);

    for (station, temperature) in iter {
        let Stat {
            min,
            max,
            sum,
            count,
        } = match stats.get_mut(station) {
            Some(x) => x,
            None => stats.entry(station.into()).or_default(),
        };
        *min = (*min).min(temperature);
        *max = (*max).max(temperature);
        *sum += i64::from(temperature);
        *count += 1;
    }

    mprint(&stats);

    // hash_stats(&stats);
}

fn hash_stats(stats: &HashMap<ArrayType, Stat, MHasher>) {
    println!();
    let mut ret = HashMap::new();

    for k in stats.keys() {
        let c: &mut usize = ret.entry(MHasher::default().hash_one(k)).or_default();
        *c += 1usize;
    }
    println!("size:{}", ret.len());

    let max = *ret.values().max().unwrap();
    let mean = ret.values().sum::<usize>() as f64 / (ret.len() as f64);
    println!("max: {max}, mean: {mean}")
}

fn mas_slice(x: &ArrayType) -> &[u8] {
    x
}

fn mprint(stats: &HashMap<ArrayType, Stat, MHasher>) {
    let mut all: Vec<(&[u8], &Stat)> = stats.iter().map(|(x, v)| (mas_slice(x), v)).collect();
    all.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

    print!("{{");

    let last = all.pop().unwrap();
    for (station, stat) in all {
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(station) };
        print!("{station}={stat}, ")
    }
    {
        let (station, stat) = last;
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(station) };
        println!("{station}={stat}}}")
    }
}

fn parse_value(str: &[u8]) -> fsize {
    let n = str.len();
    // Assuming str is verified to be valid input, checking index 0 is safe
    let sign = unsafe { *str.get_unchecked(0) } == b'-';

    // let has_4th = (n == 5) | ((n == 4) & !sign);
    let has_4th = ((n & 4) != 0) & (((n & 1) != 0) | !sign);


    // We use & 0x0F instead of wrapping_sub(b'0')
    unsafe {
        let dec = (*str.get_unchecked(n - 1) & 0x0F) as fsize;
        let unit = (*str.get_unchecked(n - 3) & 0x0F) as fsize;
        
        // Safety: If n=3, saturating_sub(4) = 0. We read str[0].
        // This is safe (len is 3). The value is garbage for this calculation, 
        // but 'has_4th' is 0, so it will be multiplied by zero and discarded.
        let ten = (*str.get_unchecked(n.saturating_sub(4)) & 0x0F) as fsize;

        let res = dec + 10 * unit + 100 * ten * (has_4th as fsize);

        // Create a mask: 0000... if positive, 1111... (-1) if negative
        let mask = -(sign as fsize); 
        (res ^ mask) - mask
    }
}

fn get_value(str: &[u8], idx: usize) -> fsize {
    debug_assert!(idx < str.len());
    let c = unsafe { str.get_unchecked(idx) };
    c.wrapping_sub(b'0') as fsize
}

impl Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Stat {
            min,
            max,
            sum,
            count,
        } = *self;
        let mean = (sum as f64) / (10. * count as f64);
        let min = (min as f64) / 10.;
        let max = (max as f64) / 10.;
        // safe
        write!(f, "{min:.1}/{mean:.1}/{max:.1}")
    }
}

struct Finder<'a> {
    data: &'a [u8],
}

impl<'a> Finder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> Iterator for Finder<'a> {
    type Item = (&'a [u8], i16);

    fn next(&mut self) -> Option<Self::Item> {
        let (station_idx, temperature_idx) = find_next(self.data)?;

        let station = &self.data[0..station_idx];
        let temperature = &self.data[station_idx + 1..station_idx + temperature_idx + 1];
        let temperature = parse_value(temperature);

        self.data = &self.data[station_idx + temperature_idx + 2..];

        Some((station, temperature))
    }
}

#[allow(nonstandard_style)]
type u8xx = u8x16;
static NUMER_SKIPPED: usize = u8xx::LEN;

fn find_next(data: &[u8]) -> Option<(usize, usize)> {
    let delimiter_nl = u8xx::splat(b'\n');
    let delimiter_sc = u8xx::splat(b';');
    let line = u8xx::load_or_default(data);

    let ridxs;
    let ridxt;
    if let Some(idxs) = delimiter_sc.simd_eq(line).first_set() {
        ridxs = idxs;
        if let Some(idxt) = delimiter_nl.simd_eq(line).first_set() {
            ridxt = idxt - idxs - 1;
        } else {
            ridxt = find_temperature_short(&data[idxs + 1..]);
        }
    } else {
        ridxs = find_station_slow(data)?;
        ridxt = find_temperature_short(&data[ridxs + 1..]);
    }
    Some((ridxs, ridxt))
}

fn find_station_slow(data: &[u8]) -> Option<usize> {
    if data.len() >= NUMER_SKIPPED {
        Some(memchr::memchr(b';', &data[NUMER_SKIPPED - 1..])? + NUMER_SKIPPED - 1)
    } else {
        None
    }
}

fn find_temperature_short(data: &[u8]) -> usize {
    let delimiter = u8x8::splat(b'\n'); // 5 max
    let line = u8x8::load_or_default(data);
    let delimeq = delimiter.simd_eq(line);
    // We know
    unsafe { delimeq.first_set().unwrap_unchecked() }
}

#[cfg(test)]
mod test {
    use crate::{Finder, parse_value};

    #[test]
    fn parse_value_sound() {
        let values = [
            "-4.5", "78.0", "0.1", "-0.0", "99.9", "2.5", "-2.5", "-99.9",
        ];

        for v in values {
            let pv = parse_value(v.as_bytes());
            let truev: f64 = v.parse().unwrap();
            assert_eq!(truev, (pv as f64) / 10.)
        }
    }

    #[test]
    fn iter_sound() {
        let values = "atr;-4.5\nrrr;78.0\nasdf;0.1\ndsaf;-0.0\n".as_bytes();

        let finder = Finder::new(values);

        for (s, t) in finder {
            dbg!(str::from_utf8(s).unwrap());
            dbg!(t);
        }
    }
}
