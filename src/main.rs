#![feature(portable_simd)]
use std::{
    collections::{HashMap, hash_map::Entry}, fmt::Display, fs::{self, File}, hash::{BuildHasher, Hasher}, io::{BufRead, BufReader}, os::linux::raw::stat, simd::{prelude::SimdPartialEq, u8x8}, str::Chars
};

use memchr::Memchr2;
use memmap2::{Mmap, MmapOptions};
use rustc_hash::FxHashMap;

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

fn main() {
    let f = File::open("measurements.txt").unwrap();
    let f = unsafe { Mmap::map(&f).unwrap() };
    f.advise(memmap2::Advice::Sequential).unwrap();

    let mut stats = HashMap::with_capacity_and_hasher(10000, rustc_hash::FxBuildHasher);
    let iter = Finder::new(&f);

    for (station, temperature) in iter {
        let Stat {
            min,
            max,
            sum,
            count,
        } =// stats.entry(station).or_default();
            match stats.get_mut(station) {
        Some(x) => x,
        None => {
           stats.entry(station.to_vec()).or_default()
        }
    };
        *min = (*min).min(temperature);
        *max = (*max).max(temperature);
        *sum += i64::from(temperature);
        *count += 1;
    }

    let mut all: Vec<(Vec<u8>, Stat)> = stats.into_iter().collect();
    all.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

    print!("{{");

    let last = all.pop().unwrap();
    for (station, stat) in all {
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(&station) };
        print!("{station}={stat}, ")
    }
    {
        let (station, stat) = last;
        // safe
        let station = unsafe { ::std::str::from_utf8_unchecked(&station) };
        print!("{station}:={stat}}}")
    }
}

fn parse_value(str: &[u8]) -> fsize {
    let n = str.len();
    let sign = str[0] == b'-';
    let has_4th = !sign && (n >= 4);
    let res = get_value(str[n - 1])
        + 10 * get_value(str[n - 3])
        + (has_4th as fsize) * 100 * get_value(str[n.saturating_sub(4)]);

    res - 2 * (sign as fsize) * res
}

fn get_value(c: u8) -> fsize {
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
        let station_idx = memchr::memchr(b';', self.data)?;
        let ndata = &self.data[station_idx + 1..];

        // let station_idx;

        // {
        // let delimiter = u8x8::splat(b'\n'); // 5 max

        // }


        // with simd
        let delimiter = u8x8::splat(b'\n'); // 5 max
        let line = u8x8::load_or_default(ndata);
        let delimeq =  delimiter.simd_eq(line);
        // We know
        let temperature_idx = unsafe{ delimeq.first_set().unwrap_unchecked()};
        // let temperature_idx = memchr::memchr(b'\n', ndata)?;

        let station = &self.data[0..station_idx];
        let temperature = &ndata[0..temperature_idx];
        let temperature = parse_value(temperature);

        self.data = &ndata[temperature_idx + 1..];

        Some((station, temperature))
    }
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
