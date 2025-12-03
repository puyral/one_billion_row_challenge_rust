use std::fmt::Display;

use crate::fsize;

#[derive(Clone, Copy)]
pub struct Stat {
    pub min: fsize,
    pub max: fsize,
    pub sum: i64,
    pub count: u32,
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

impl Stat {
    pub fn reduce(iter: impl IntoIterator<Item = Self>) -> Option<Self> {
        iter.into_iter().reduce(|a, b| Self {
            min: a.min.min(b.min),
            max: a.max.max(b.max),
            sum: a.sum + b.sum,
            count: a.count + b.count,
        })
    }
}
