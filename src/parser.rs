use std::{
    hint::unreachable_unchecked,
    simd::{
        Mask, Simd, i16x4,
        prelude::{SimdInt, SimdPartialEq},
        u8x4, u8x8, u8x16, u8x32, u64x4, usizex4,
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
        let temperature = parse_value(data.as_ptr(), temperature_idx, temperature_end_idx);

        *current = temperature_end_idx + 1;
        Some((station, temperature))
    }
}

#[allow(nonstandard_style)]
type u8xx = u8x32;
static NUMER_SKIPPED: usize = u8xx::LEN;

fn find_next(data: &[u8]) -> Option<(usize, usize)> {
    let delimiter_nl = u8xx::splat(b'\n');
    let delimiter_sc = u8xx::splat(b';');
    let line = u8xx::load_or_default(data);

    let sc = delimiter_sc.simd_eq(line).first_set();
    let nl = delimiter_nl.simd_eq(line).first_set();

    match (sc, nl) {
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

    // if let Some(idxs) = delimiter_sc.simd_eq(line).first_set() {
    //     ridxs = idxs;
    //     if let Some(idxt) = delimiter_nl.simd_eq(line).first_set() {
    //         ridxt = idxt - idxs - 1;
    //     } else {
    //         ridxt = find_temperature_short(&data[idxs + 1..]);
    //     }
    // } else {
    //     ridxs = find_station_slow(data)?;
    //     ridxt = find_temperature_short(&data[ridxs + 1..]);
    // }
    // Some((ridxs, ridxt))
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

 fn parse_value(str: *const u8, start: usize, end: usize) -> fsize {
    unsafe {
        let n = end - start;
        let sign = *(str.add(start)) == b'-';
        // dbg!((*(str.add(end-1)) as u8 ).as_ascii_unchecked());
        let has_4th = ((n & 4) != 0) & (((n & 1) != 0) | !sign);
        // let mult = i16x4::from_array([1, 0, 10 /* the '.' */, 100 * (has_4th as fsize)]);

        // let indices = usizex4::from_array(::std::array::from_fn(|i| end - i));

        // let values = ::std::array::from_fn(|i| str[end - i - 1] as i16);
        // let values = i16x4::from_array(values);

        // let values = (values & i16x4::splat(0x0F)) * mult;
        // let res = values.reduce_sum();

        // let mut res = 0;

        // for (i, mult) in [(1, 1), (3, 10), (4, 100 * (has_4th as fsize))] {
        //     let v = unsafe { str.get_unchecked(end - i) };
        //     res += (*v & 0x0F) as fsize * mult
        // }

        // let mask = -(sign as fsize);
        // (res ^ mask) - mask

        // We use & 0x0F instead of wrapping_sub(b'0')
        let mut end = str.add(end-4);

        let ten = ((*end) & 0x0F) as fsize;
        let unit = ((*end.add(4-3)) & 0x0F) as fsize;
        let dec = ((*end.add(4-1)) & 0x0F) as fsize;
        // dbg!((dec as u8 + b'0').as_ascii_unchecked());
        // dbg!((unit as u8 + b'0').as_ascii_unchecked());
        // dbg!((ten as u8 + b'0').as_ascii_unchecked());

        // Safety: If n=3, saturating_sub(4) = 0. We read str[0].
        // This is safe (len is 3). The value is garbage for this calculation,
        // but 'has_4th' is 0, so it will be multiplied by zero and discarded.

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
            let nv = format!(";{v}");
            let pv = parse_value(nv.as_bytes().as_ptr(), 1, 1 + nv.len()-1);
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
