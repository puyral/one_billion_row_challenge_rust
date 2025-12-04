use std::{
    hint::unreachable_unchecked,
    simd::{
        Mask, Simd, i16x4,
        prelude::{SimdInt, SimdPartialEq, SimdUint},
        u8x4, u8x8, u8x16, u8x32, u16x4, u64x4, usizex4,
    },
};

use crate::fsize;

pub struct Finder<'a> {
    data: &'a [u8],
    current: usize,
    end: usize,
}

impl<'a> Finder<'a> {
    pub fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        assert!(data.len() > start);
        assert!(data.len() > end);
        Self {
            data,
            current: start,
            end,
        }
    }
}

impl<'a> Iterator for Finder<'a> {
    type Item = (&'a [u8], i16);

    #[inline(never)]
    fn next(&mut self) -> Option<Self::Item> {
        let Self { data, current, end } = self;
        if *end < *current {
            return None;
        }
        let (station_end_idx, temperature_end_idx) = find_next(&data[*current..])?;
        let station_end_idx = *current + station_end_idx;
        let temperature_end_idx = *current + temperature_end_idx;

        let station = &data[*current..station_end_idx];
        let temperature_idx = station_end_idx + 1;
        let temperature = parse_value(data, temperature_idx, temperature_end_idx);

        *current = temperature_end_idx + 1;
        Some((station, temperature))
    }
}

#[allow(nonstandard_style)]
type u8xx = u8x32;
static NUMER_SKIPPED: usize = u8xx::LEN;

fn find_next(data: &[u8]) -> Option<(usize, usize)> {
    match simd_search(data) {
        (Some(idsc), Some(idnl)) => Some((idsc, idnl)),
        (Some(idsc), None) => {
            let idnl = find_temperature_short(&data[idsc + 1..]) + idsc + 1;
            Some((idsc, idnl))
        }
        (None, None) => {
            let idsc = find_station_slow(data)?;
            let idnl = find_temperature_short(&data[idsc + 1..]) + idsc + 1;
            Some((idsc, idnl))
        }
        _ => unsafe { unreachable_unchecked() },
    }
}

fn simd_search(data: &[u8]) -> (Option<usize>, Option<usize>) {
    let delimiter_nl = u8xx::splat(b'\n');
    let delimiter_sc = u8xx::splat(b';');
    let line = u8xx::load_or_default(data);

    let sc = delimiter_sc.simd_eq(line).first_set();
    let nl = delimiter_nl.simd_eq(line).first_set();
    (sc, nl)
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

fn compute_shape(str: &[u8], start: usize, end: usize) -> (bool, bool) {

    let n = end - start;
    let sign = str[start] == b'-';
    let has_4th = ((n & 4) != 0) & (((n & 1) != 0) | !sign);
    (sign, has_4th)
}

// #[inline(never)]
fn parse_value(str: &[u8], start: usize, end: usize) -> fsize {
    let (sign, has_4th) = compute_shape(str, start, end);

    let res: i16 = [(1, 1), (3, 10), (4, 100 * (has_4th as fsize))]
        .into_iter()
        .map(|(i, mul)| {
            let v = unsafe { str.get_unchecked(end - i) };
            (*v & 0x0F) as fsize * mul
        })
        .sum();

    let mask = -(sign as fsize);
    (res ^ mask) - mask
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
            let nv = format!(";{v}");
            let pv = parse_value(nv.as_bytes(), 1, nv.as_bytes().len());
            let truev: f64 = v.parse().unwrap();
            assert_eq!(truev, (pv as f64) / 10.)
        }
    }

    #[test]
    fn iter_sound() {
        let values = "atr;-4.5\nrrr;78.0\nasdf;0.1\ndsaf;-0.0\n".as_bytes();

        let finder = Finder::new(values, 0, values.len() - 1);

        for (s, t) in finder {
            dbg!(str::from_utf8(s).unwrap());
            dbg!(t);
        }
    }
}
