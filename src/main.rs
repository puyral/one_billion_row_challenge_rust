use std::{
    collections::{HashMap, hash_map::Entry},
    fs::File,
    io::{BufRead, BufReader},
};

#[allow(nonstandard_style)]
type fsize = f32;

struct Stat {
    min: fsize,
    max: fsize,
    sum: fsize,
    count: u32,
}

impl Default for Stat {
    fn default() -> Self {
        Self {
            min: fsize::MAX,
            max: fsize::MIN,
            sum: 0.,
            count: 0,
        }
    }
}

fn main() {
    println!("Hello, world!");

    let f = File::open("measurements.txt").unwrap();
    let f = BufReader::new(f);

    let mut stats = HashMap::with_capacity(10000);

    for line in f.lines() {
        let l = String::leak(line.unwrap());
        let (station, temperature) = l.split_once(';').unwrap();
        let temperature = temperature.parse().unwrap();
        let Stat {
            min,
            max,
            sum,
            count,
        } = stats.entry(station).or_default();
        *min = min.min(temperature);
        *max = max.max(temperature);
        *sum += temperature;
        *count += 1;
    }

    let mut all: Vec<_> = stats.into_iter().collect();
    all.sort_unstable_by_key(|(k, _)| *k);

    print!("{{");

    let last = all.pop().unwrap();
    for (
        station,
        Stat {
            min,
            max,
            sum,
            count,
        },
    ) in all
    {
        let mean = sum / (count as fsize);
        print!("{station}={min:.1}/{mean:.1}/{max:.1}, ")
    }
    {
        let (
            station,
            Stat {
                min,
                max,
                sum,
                count,
            },
        ) = last;
        let mean = sum / (count as fsize);
        print!("{station}:={min:.1}/{mean:.1}/{max:.1}}}")
    }
}
