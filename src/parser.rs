use std::{
    hint::unreachable_unchecked,
    os::unix::raw::off_t,
    simd::{
        Mask, Simd, i16x4,
        prelude::{SimdInt, SimdPartialEq, SimdUint},
        u8x4, u8x8, u8x16, u8x32, u16x4, u64x4, usizex4,
    },
    usize,
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

/// repeats `e` for the while number
macro_rules! mk_splat {
    ($t:ident; $e:expr) => {
        $t::from_ne_bytes([$e; _])
    };
}

static SWAR_STATION: bool = false;

#[inline(never)]
fn find_next(data: &[u8]) -> Option<(usize, usize)> {
    if SWAR_STATION {
        // deactivated
        match sawr_station_search(data) {
            Some(idsc) => Some((idsc, find_temperature(data, idsc))),
            None if data.len() > NUMER_SKIPPED => slow_search(data, NUMER_SKIPPED),
            _ => None,
        }
    } else {
        #[allow(clippy::collapsible_else_if)]
        if data.len() < MIN_SIMD_LEN {
            // rare slow path
            let idsc = data.iter().position(|x| *x == b';')?;
            Some((idsc, find_temperature(data, idsc)))
        } else {
            simd_search(data)
        }
    }
}

static MIN_SIMD_LEN: usize = 100_usize.div_ceil(u8xx::LEN) * u8xx::LEN;
fn simd_search(data: &[u8]) -> Option<(usize, usize)> {
    assert!(data.len() >= MIN_SIMD_LEN);
    let upper = MIN_SIMD_LEN / u8xx::LEN;
    let delimiter_nl = u8xx::splat(b'\n');
    let delimiter_sc = u8xx::splat(b';');

    for i in 0..upper {
        let offset = i * u8xx::LEN;
        // because of the assertion
        let line = unsafe {
            u8xx::load_select_unchecked(&data[offset..], Mask::splat(true), u8xx::splat(0))
        };

        let sc = delimiter_sc.simd_eq(line).first_set();
        let nl = delimiter_nl.simd_eq(line).first_set();

        match (sc, nl) {
            (Some(idsc), Some(idnl)) => return Some((offset + idsc, offset + idnl)),
            (Some(idsc), None) => {
                return Some((idsc, find_temperature(data, offset + idsc)));
            }
            (None, None) => continue,
            _ => unsafe { unreachable_unchecked() },
        }
    }
    None
}

fn slow_search(data: &[u8], skipped: usize) -> Option<(usize, usize)> {
    let idsc = memchr::memchr(b';', &data[skipped - 1..])? + skipped - 1;
    let idnl = find_temperature(data, idsc);
    Some((idsc, idnl))
}

fn find_temperature(data: &[u8], offset: usize) -> usize {
    let offset = offset + 1;
    let data = &data[offset..];

    let res = if data.len() < 8 {
        // nearly never run
        data.iter().position(|x| *x == b'\n').unwrap()
    } else {
        let data: &[u8] = data;
        debug_assert!(data.len() >= 8);
        // SAFETY: We assume the buffer has at least 8 bytes available
        let chunk = unsafe { (data.as_ptr() as *const u64).read_unaligned() };
        let pattern = mk_splat!(u64; b'\n');
        let xored = chunk ^ pattern;
        let low_magic = mk_splat!(u64; 0x01);
        let high_magic = mk_splat!(u64; 0x80);
        let mask = (xored.wrapping_sub(low_magic)) & !xored & high_magic;
        (mask.trailing_zeros() >> 3) as usize
    };
    res + offset
}

fn sawr_station_search(data: &[u8]) -> Option<usize> {
    assert_eq!(NUMER_SKIPPED, ::std::mem::size_of::<u128>());
    if data.len() < NUMER_SKIPPED {
        // slow final path
        return data.iter().position(|x| *x == b';');
    }

    let chunk = unsafe { (data.as_ptr() as *const u128).read_unaligned() };
    let pattern = mk_splat!(u128; b';');
    let xored = chunk ^ pattern;

    // (v - 0x01) & !v & 0x80 detects zero bytes.
    let low_magic = mk_splat!(u128; 0x01);
    let high_magic = mk_splat!(u128; 0x80);

    // This results in 0x80 in the byte slot where \n was, and 0x00 elsewhere.
    let mask = (xored.wrapping_sub(low_magic)) & !xored & high_magic;

    if mask == 0 {
        None
    } else {
        Some((mask.trailing_zeros() / 8) as usize)
    }
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
