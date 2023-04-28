use std::{collections::HashMap, hash::Hash, io, iter, ops::RangeToInclusive};

use itertools::Itertools;
use rand::{distributions::Standard, thread_rng, Rng, RngCore};
use statrs::distribution::{ChiSquared, ContinuousCDF};

use crate::{
    acc::{Accumulator, NumCooccurrences, NumOccurrences},
    algo::Algorithm,
};

pub(crate) trait Experiment {
    type Accumulator: Accumulator;

    fn new_accumulator(&self) -> Self::Accumulator;

    fn run(&self, accumulator: &mut Self::Accumulator, algorithm: &impl Algorithm);

    fn accumulate(&self, algorithm: &impl Algorithm, num_iterations: u64) -> Self::Accumulator {
        let mut accumulator = self.new_accumulator();
        for _ in 0..num_iterations {
            self.run(&mut accumulator, algorithm);
        }
        accumulator
    }

    fn write_summary(
        &self,
        output: &mut impl io::Write,
        accumulator: &Self::Accumulator,
    ) -> Result<(), std::io::Error>;
}

#[derive(Clone, Debug)]
pub(crate) struct Regularity {
    range: RangeToInclusive<u64>,
    input_size_bytes: usize,
}

impl Regularity {
    pub(crate) fn new(range: RangeToInclusive<u64>, input_size_bytes: usize) -> Self {
        Self {
            range,
            input_size_bytes,
        }
    }
}

impl Experiment for Regularity {
    type Accumulator = NumOccurrences<u64>;

    fn new_accumulator(&self) -> Self::Accumulator {
        NumOccurrences::new(
            usize::try_from(self.range.end)
                .unwrap()
                .checked_add(1)
                .unwrap(),
        )
    }

    #[inline]
    fn run(&self, accumulator: &mut Self::Accumulator, algorithm: &impl Algorithm) {
        let mut bytes = vec![0; self.input_size_bytes];
        thread_rng().fill_bytes(&mut bytes);
        let hash = algorithm.hash(&bytes, 0, self.range);
        accumulator.record(hash);
    }

    fn write_summary(
        &self,
        output: &mut impl io::Write,
        accumulator: &Self::Accumulator,
    ) -> Result<(), std::io::Error> {
        let num_keys = accumulator.num_iterations();
        let range_len = accumulator.counts().len();
        let l1_distance = accumulator
            .counts()
            .iter()
            .map(|&c| c as f64 / num_keys as f64)
            .map(|p| (p - 1.0 / range_len as f64).abs())
            .sum::<f64>();
        let l2_distance = accumulator
            .counts()
            .iter()
            .map(|&c| c as f64 / num_keys as f64)
            .map(|p| (p - 1.0 / range_len as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        let p_value = chi_squared_uniformity_test_p_value(accumulator.counts());
        output.write_fmt(format_args!(
            ", \"num keys\": {num_keys}\
            , \"l1 distance\": {l1_distance:e}\
            , \"l2 distance\": {l2_distance:e}\
            , \"p-value\": {p_value}"
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Collisions {
    range: RangeToInclusive<u64>,
    input_size_bytes: usize,
}

impl Collisions {
    pub(crate) fn new(range: RangeToInclusive<u64>, input_size_bytes: usize) -> Self {
        Self {
            range,
            input_size_bytes,
        }
    }
}

impl Experiment for Collisions {
    type Accumulator = NumOccurrences<u64>;

    fn new_accumulator(&self) -> Self::Accumulator {
        NumOccurrences::new(
            usize::try_from(self.range.end)
                .unwrap()
                .checked_add(1)
                .unwrap(),
        )
    }

    #[inline]
    fn run(&self, accumulator: &mut Self::Accumulator, algorithm: &impl Algorithm) {
        let mut bytes = vec![0; self.input_size_bytes];
        thread_rng().fill_bytes(&mut bytes);
        let hash = algorithm.hash(&bytes, 0, self.range);
        accumulator.record(hash);
    }

    fn write_summary(
        &self,
        output: &mut impl io::Write,
        accumulator: &Self::Accumulator,
    ) -> Result<(), std::io::Error> {
        let num_keys = accumulator.num_iterations();
        let num_collisions = accumulator
            .counts()
            .iter()
            .filter(|&&c| c > 1)
            .map(|&c| c as f64)
            .map(|c| c * (c - 1.0) / 2.0)
            .sum::<f64>();
        let c_hat = num_collisions / (num_keys as f64 * (num_keys as f64 - 1.0) / 2.0);
        let normalized_c_hat = c_hat * accumulator.counts().len() as f64;
        output.write_fmt(format_args!(
            ", \"num keys\": {num_keys}\
            , \"num collisions\": {num_collisions:e}\
            , \"c hat\": {c_hat:e}\
            , \"normalized c hat\": {normalized_c_hat:e}"
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct IndependenceAcrossRanges {
    ranges: Vec<RangeToInclusive<u64>>,
    input_size_bytes: usize,
}

impl IndependenceAcrossRanges {
    pub(crate) fn new(ranges: Vec<RangeToInclusive<u64>>, input_size_bytes: usize) -> Self {
        assert!(ranges.iter().all_unique());
        Self {
            ranges,
            input_size_bytes,
        }
    }
}

impl Experiment for IndependenceAcrossRanges {
    type Accumulator = NumCooccurrences<u64>;

    fn new_accumulator(&self) -> Self::Accumulator {
        NumCooccurrences::new(
            iter::once(0..=self.ranges[0].end)
                .chain(
                    self.ranges
                        .iter()
                        .tuple_windows()
                        .map(|(&r0, &r1)| r0.end + 1..=r1.end),
                )
                .multi_cartesian_product(),
        )
    }

    #[inline]
    fn run(&self, accumulator: &mut Self::Accumulator, algorithm: &impl Algorithm) {
        let mut bytes = vec![0; self.input_size_bytes];
        loop {
            thread_rng().fill_bytes(&mut bytes);
            let hashes = self
                .ranges
                .iter()
                .map(|&range| algorithm.hash(&bytes, 0, range))
                .collect::<Vec<_>>();
            if hashes.iter().all_unique() {
                accumulator.record(hashes);
                break;
            }
        }
    }

    fn write_summary(
        &self,
        output: &mut impl io::Write,
        accumulator: &Self::Accumulator,
    ) -> Result<(), std::io::Error> {
        let num_keys = accumulator.num_iterations();
        let p_value =
            chi_squared_mutual_independence_test_p_value(accumulator.counts(), self.ranges.len());
        output.write_fmt(format_args!(
            ", \"num keys\": {num_keys}\
            , \"p-value\": {p_value}"
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct IndependenceAcrossSeeds {
    range: RangeToInclusive<u64>,
    seeds: Vec<u64>,
    input_size_bytes: usize,
}

impl IndependenceAcrossSeeds {
    pub(crate) fn new(
        range: RangeToInclusive<u64>,
        num_seeds: usize,
        input_size_bytes: usize,
    ) -> Self {
        Self {
            range,
            seeds: iter::repeat_with(|| {
                thread_rng()
                    .sample_iter(Standard)
                    .take(num_seeds)
                    .collect::<Vec<_>>()
            })
            .find(|seeds| seeds.iter().all_unique())
            .unwrap(),
            input_size_bytes,
        }
    }
}

impl Experiment for IndependenceAcrossSeeds {
    type Accumulator = NumCooccurrences<u64>;

    fn new_accumulator(&self) -> Self::Accumulator {
        NumCooccurrences::new(
            iter::repeat(0..=self.range.end)
                .take(self.seeds.len())
                .multi_cartesian_product(),
        )
    }

    #[inline]
    fn run(&self, accumulator: &mut Self::Accumulator, algorithm: &impl Algorithm) {
        let mut bytes = vec![0; self.input_size_bytes];
        thread_rng().fill_bytes(&mut bytes);
        let hashes = self
            .seeds
            .iter()
            .map(|&seed| algorithm.hash(&bytes, seed, self.range))
            .collect::<Vec<_>>();
        accumulator.record(hashes)
    }

    fn write_summary(
        &self,
        output: &mut impl io::Write,
        accumulator: &Self::Accumulator,
    ) -> Result<(), std::io::Error> {
        let num_keys = accumulator.num_iterations();
        let p_value =
            chi_squared_mutual_independence_test_p_value(accumulator.counts(), self.seeds.len());
        output.write_fmt(format_args!(
            ", \"num keys\": {num_keys}\
            , \"p-value\": {p_value}"
        ))
    }
}

fn chi_squared_uniformity_test_p_value(num_occurrences: &Vec<u64>) -> f64 {
    let expected_count = num_occurrences.iter().sum::<u64>() as f64 / num_occurrences.len() as f64;

    let statistic = num_occurrences
        .iter()
        .map(|&o| (o as f64 - expected_count).powi(2) / expected_count)
        .sum::<f64>();

    let degrees_of_freedom = num_occurrences.len() as f64 - 1.0;

    1.0 - ChiSquared::new(degrees_of_freedom).unwrap().cdf(statistic)
}

fn chi_squared_mutual_independence_test_p_value<H: Eq + Hash>(
    num_cooccurrences: &HashMap<Vec<H>, u64>,
    n: usize,
) -> f64 {
    let (marginal_probabilities, num_samples) = {
        let mut p = iter::repeat_with(HashMap::<_, f64>::new)
            .take(n)
            .collect::<Vec<_>>();
        num_cooccurrences.iter().for_each(|(i, &v)| {
            iter::zip(i, &mut p).for_each(|(i_i, p_i)| *p_i.entry(i_i).or_default() += v as f64);
        });
        let n = p[0].values().sum::<f64>();
        p.iter_mut()
            .flat_map(|p_i| p_i.values_mut())
            .for_each(|p| *p /= n);
        p.iter()
            .for_each(|p_i| assert!((p_i.values().sum::<f64>() - 1.0).abs() < 1e-2));
        (p, n)
    };

    let statistic = num_cooccurrences
        .iter()
        .map(|(i, &o)| {
            let joint_probability = iter::zip(&marginal_probabilities, i)
                .map(|(p_i, i_i)| *p_i.get(&i_i).unwrap())
                .product::<f64>();
            let e = joint_probability * num_samples;
            (o as f64 - e).powi(2) / e
        })
        .sum::<f64>();

    let degrees_of_freedom = (marginal_probabilities
        .iter()
        .map(HashMap::len)
        .product::<usize>()
        - 1)
        - (marginal_probabilities
            .iter()
            .map(HashMap::len)
            .map(|len| len - 1)
            .sum::<usize>());

    assert!(degrees_of_freedom > 0);
    1.0 - ChiSquared::new(degrees_of_freedom as f64)
        .unwrap()
        .cdf(statistic)
}
