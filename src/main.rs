#![feature(portable_simd)]
#![feature(hasher_prefixfree_extras)]
#![feature(int_lowest_highest_one)]
#![allow(unused)]
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fmt::Display,
    fs::{self, File},
    hash::{BuildHasher, Hash, Hasher},
    io::{BufRead, BufReader},
    simd::{prelude::SimdPartialEq, u8x4, u8x8, u8x16, u8x32, u8x64},
    str::Chars,
    thread,
};

use memchr::Memchr2;
use memmap2::{Mmap, MmapOptions};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::{SmallVec, ToSmallVec};

mod hasher;
use hasher::MHasher;

mod parser;
use parser::Finder;

mod stats;
use stats::Stat;

use crate::hashmap::StackMap;

mod hashmap;

#[allow(nonstandard_style)]
type fsize = i16;

// type ArrayType = SmallVec<[u8; 16]>;
type ArrayType = Box<[u8]>;

static FILE: &str = match option_env!("FILE") {
    Some(x) => x,
    None => "measurements.txt",
};

fn main() {
    let f = File::open(FILE).unwrap();
    let f = unsafe { Mmap::map(&f).unwrap() };
    f.advise(memmap2::Advice::Sequential).unwrap();

    // set the NUM_CPU env variable at compile time to change the number of cpu used. Defaults choosing the max at runtime
    let n_cpus = option_env!("NUM_CPU")
        .map(|x| x.parse().unwrap())
        .unwrap_or(num_cpus::get());

    let chunk_size = f.len() / (n_cpus);
    println!("memory usage: {}", ::std::mem::size_of::<HMap>() * n_cpus);

    let (results, mut stations_vec): (Vec<_>, Vec<_>) = thread::scope(|sc| {
        let handles: Vec<_> = (0..n_cpus)
            .map(|i| {
                // Create a builder with custom stack size
                std::thread::Builder::new()
                    .name(format!("worker-{}", i)) // Optional: helps with debugging
                    .stack_size(usize::max(
                        32 * 1024 * 1024,
                        2 * ::std::mem::size_of::<HMap>(),
                    )) // Set to 32MB (adjust as needed)
                    .spawn_scoped(sc, {
                        let f = &f;
                        move || process(f, i, chunk_size, i +1== n_cpus)
                    })
                    .expect("failed to spawn thread") // Builder returns a Result
            })
            .collect::<Vec<_>>();
        handles.into_iter().map(|h| h.join().unwrap()).unzip()
    });
    let mut stations = HashSet::with_capacity_and_hasher(
        stations_vec.iter().map(|x| x.len()).max().unwrap_or(1) * 2,
        FxBuildHasher,
    );

    while let Some(s) = stations_vec.pop() {
        for station in s {
            for other in &mut stations_vec {
                other.remove(&station);
            }
            stations.insert(station);
        }
    }

    mprint(&results, &stations);
}

fn process(f: &[u8], n: usize, chunk_size: usize, last: bool) -> (HMap, FxHashSet<ArrayType>) {
    let iter = {
        let start = refine_start(f, n * chunk_size);
        let f = &f[start..];
        let size = if last {
            f.len()
        } else {
            chunk_size - (n * chunk_size - start)
        };
        Finder::new(f, size)
    };

    let mut stats = init_map();
    for (station, temperature) in iter {
        let Stat {
            min,
            max,
            sum,
            count,
        } = match stats.get_mut(station) {
            Some(x) => x,
            None => insert_or_default(&mut stats, station),
        };
        *min = (*min).min(temperature);
        *max = (*max).max(temperature);
        *sum += i64::from(temperature);
        *count += 1;
    }

    let keys = stats.keys().cloned().collect();
    (stats, keys)
}

fn refine_start(f: &[u8], start: usize) -> usize {
    if start == 0 || f[start - 1] == b'\n' {
        start
    } else {
        start + memchr::memchr(b'\n', &f[start..]).unwrap() + 1
    }
}

/// outputs the results
fn mprint(stats: &[HMap], stations: &FxHashSet<ArrayType>) {
    let mut all: Vec<_> = stations.iter().collect();
    all.sort_unstable();

    let f = |s| {
        (
            mas_slice(s),
            Stat::reduce(stats.iter().map(|m| m.get(s).copied().unwrap_or_default()))
                .unwrap_or_default(),
        )
    };

    let last = all.pop().unwrap();
    let all = all.into_iter().map(&f).peekable();

    print!("{{");

    for (station, stat) in all {
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(station) };
        print!("{station}={stat}, ")
    }
    {
        let (station, stat) = f(last);
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(station) };
        println!("{station}={stat}}}")
    }
}

/// To be able to more easly swap the array type
fn mas_slice(x: &ArrayType) -> &[u8] {
    x
}

// type HMap = HashMap<ArrayType, Stat, MHasher>;
type HMap = StackMap<ArrayType, Stat, MHasher>;

fn init_map() -> HMap {
    // HashMap::with_capacity_and_hasher(10000, MHasher::default())
    HMap::new()
}

fn insert_or_default<'a>(stats: &'a mut HMap, station: &[u8]) -> &'a mut Stat {
    // stats.entry(station.into()).or_default()
    stats.insert(station.into(), Default::default())
}

trait HashStat {
    fn hash_stats(stats: &Self);
}

impl HashStat for HashMap<ArrayType, Stat, MHasher> {
    /// prints stats about the hash
    fn hash_stats(stats: &Self) {
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
}
