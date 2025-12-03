use std::simd::{prelude::SimdPartialEq, u8x8, u8x16};

use crate::fsize;

pub struct Finder<'a> {
    data: &'a [u8],
    /// The size of what will be left in `data` once we are done
    end_length: usize
}

impl<'a> Finder<'a> {
    pub fn new(data: &'a [u8], size: usize) -> Self {
        assert!(data.len() >= size, "len: {}, size: {size}", data.len());
        let end_length  = data.len() - size;
        Self { data , end_length}
    }
}

impl<'a> Iterator for Finder<'a> {
    type Item = (&'a [u8], i16);

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len() < self.end_length {
            return None;
        }
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

#[cfg(test)]
mod test {
    use super::{Finder, parse_value};

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

        let finder = Finder::new(values, 0);

        for (s, t) in finder {
            dbg!(str::from_utf8(s).unwrap());
            dbg!(t);
        }
    }
}
