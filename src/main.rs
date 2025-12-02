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

mod hasher;
use hasher::MHasher;

mod parser;
use parser::Finder;

mod stats;
use stats::Stat;

#[allow(nonstandard_style)]
type fsize = i16;

// type ArrayType = SmallVec<[u8; 16]>;
type ArrayType = Vec<u8>;

static FILE: &str = match option_env!("FILE") {
    Some(x) => x,
    None => "measurements.txt",
};

fn main() {
    let f = File::open(FILE).unwrap();
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

/// prints stats about the hash
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

/// outputs the results
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

/// To be able to more easly swap the array type
fn mas_slice(x: &ArrayType) -> &[u8] {
    x
}
