use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::BuildHasher;
use std::hash::Hash;

use smallvec::SmallVec;

use crate::HashStat;

static MAP_SIZE: usize = 1 << (10_000usize.highest_one().unwrap() + 1);
static MASK: usize = MAP_SIZE - 1;

static BUCKET_SIZE: usize = 1;

// struct Bucket<T>(SmallVec<[T; BUCKET_SIZE]>);

struct ContentBucket<K, V> {
    hash_mem: u64,
    key: K,
    value: V,
}

struct Bucket<K, V>(Option<ContentBucket<K, V>>);

pub struct StackMap<K, V, H> {
    content: [Bucket<K, V>; MAP_SIZE],
    hasher: H,
    size: usize,
}

impl<K, V> Default for Bucket<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K, V, H> StackMap<K, V, H> {
    pub fn new_with_hasher(hasher: H) -> Self {
        Self {
            content: ::std::array::from_fn(|_| Default::default()),
            hasher,
            size: 0,
        }
    }

    pub fn new() -> Self
    where
        H: Default,
    {
        Self::new_with_hasher(Default::default())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.content.iter().filter_map(|Bucket(b)| {
            let ContentBucket { key, value, .. } = b.as_ref()?;
            Some((key, value))
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }
}

impl<K, V> PartialEq for ContentBucket<K, V>
where
    K: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.hash_mem == other.hash_mem && self.key == other.key
    }
}

impl<K, V> Eq for ContentBucket<K, V> where K: Eq {}

impl<K, V> PartialOrd for ContentBucket<K, V>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.hash_mem.partial_cmp(&other.hash_mem) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }
        match self.key.partial_cmp(&other.key) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }
        Some(Ordering::Equal)
    }
}

impl<K, V> Ord for ContentBucket<K, V>
where
    K: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.hash_mem.cmp(&other.hash_mem) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.key.cmp(&other.key) {
            Ordering::Equal => {}
            ord => return ord,
        }
        Ordering::Equal
    }
}

impl<K, V, H> StackMap<K, V, H>
where
    K: Hash + Ord,
    H: BuildHasher,
{
    pub fn insert(&mut self, key: K, value: V) -> &mut V {
        assert!(self.size < MAP_SIZE, "no space left");

        let hashed = self.hasher.hash_one(&key);

        let mut new_bucket = ContentBucket {
            hash_mem: hashed,
            key,
            value,
        };

        let mut idx = hashed as usize & MASK;

        while let Bucket(Some(bucket)) = &mut self.content[idx] {
            match new_bucket.cmp(bucket) {
                Ordering::Equal => {
                    break;
                    // bucket.value = new_bucket.value;
                    // return  &mut bucket.value;
                    // std::mem::swap(&mut new_bucket.value, &mut bucket.value);
                    // return Some(new_bucket.value);
                }
                // keep the smallest the closest to its "true" location
                Ordering::Less => std::mem::swap(&mut new_bucket, bucket),

                Ordering::Greater => {}
            }

            // this will not loop because there is a least one free space
            idx = (idx + 1) % MAP_SIZE
        }
        self.content[idx] = Bucket(Some(new_bucket));
        self.size += 1;

        &mut unsafe { self.content[idx].0.as_mut().unwrap_unchecked() }.value
    }

    pub fn get_mut<'a, Q>(&'a mut self, key: &Q) -> Option<&'a mut V>
    where
        K: Borrow<Q>,
        Q: ?Sized,
        Q: Hash + Eq,
    {
        let hashed = self.hasher.hash_one(key);

        let mut idx = hashed as usize & MASK;
        let idx = loop {
            match &self.content[idx] {
                Bucket(None) => return None,
                Bucket(Some(ContentBucket {
                    hash_mem,
                    key: ckey,
                    ..
                })) if *hash_mem == hashed && ckey.borrow() == key => break idx,
                _ => idx = (idx + 1) % MAP_SIZE,
            }
        };
        // safety we just computed `idx` above
        let ContentBucket { value, .. } =
            unsafe { self.content[idx].0.as_mut().unwrap_unchecked() };
        Some(value)
    }
}

impl<K, V> ContentBucket<K, V> {
    fn het_cmp<Q>(&self, hashed_other: u64, key: &Q) -> Ordering
    where
        K: Borrow<Q>,
        Q: Ord,
        Q: ?Sized,
    {
        match self.hash_mem.cmp(&hashed_other) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.key.borrow().cmp(key) {
            Ordering::Equal => {}
            ord => return ord,
        }
        Ordering::Equal
    }
}

impl<K, V, H> HashStat for StackMap<K, V, H>
where
    K: Hash + Ord,
    H: BuildHasher + Default,
{
    fn hash_stats(stats: &Self) {
        println!();
        let mut ret = std::collections::HashMap::new();

        for k in stats.keys() {
            let c: &mut usize = ret
                .entry(H::default().hash_one(k) as usize & MASK)
                .or_default();
            *c += 1usize;
        }
        println!("size:{}", ret.len());

        let max = *ret.values().max().unwrap();
        let mean = ret.values().sum::<usize>() as f64 / (ret.len() as f64);
        println!("max: {max}, mean: {mean}")
    }
}
